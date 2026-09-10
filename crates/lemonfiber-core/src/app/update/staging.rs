//! Taking the steps an update proposed, one service at a time.
//!
//! The order is the safety. What is still coming down is asked about before anything
//! stops, the disk is measured before anything is pulled, the backup is taken while
//! nothing can be writing to a database, and each service is proven to answer before
//! the next one is touched. A failure at any of them stops the run where it is: a
//! single service that would not come back is diagnosable, and twelve simultaneously
//! are not.
//!
//! The whole stack comes down for the capture, because a capture is refused while
//! anything might be writing to a database — so a run that reaches the end brings it
//! back up, and one that halted leaves it where it stopped for the operator to decide
//! about with the report in front of them.
//!
//! Services are moved in the order the manifest declares them rather than in the
//! order they read, because that order is where its dependencies are written down —
//! and starting a service before the one it needs is a failure this would then
//! attribute to the wrong image.

use lemonfiber_manifest::Manifest;

use crate::docker::survey;
use crate::error::{Diagnose, Problem};
use crate::stack::compose::Action;
use crate::update::{self, Applied, Change, Ending};

use super::super::{backup, engine, space, Ctx, Waiting};
use super::{left_down, named, still_transferring, Report};

/// How often a started service is asked whether it is answering yet.
const POLL: std::time::Duration = std::time::Duration::from_millis(500);

/// Take the steps, behind the backup, and say what each of them came to.
pub(super) async fn apply(
    ctx: &Ctx,
    manifest: &Manifest,
    changes: Vec<Change>,
    wait: Waiting,
) -> Result<Report, Box<Problem>> {
    let taking = ordered(manifest, &changes);
    if taking.is_empty() {
        return Ok(Report::proposed(changes, Vec::new(), true));
    }
    // Built while nothing has been touched, so a stack that cannot be resolved or
    // written stops the run before the first service is stopped rather than after a
    // backup has been taken and half of them have moved.
    let taking = steps(ctx, taking)?;

    // Asked before anything stops, because stopping is what would interrupt them.
    let active = named(&engine::in_flight(ctx, &[]).await);
    if !active.is_empty() {
        if wait != Waiting::ForTheDownloads {
            return Err(Box::new(still_transferring(&active)));
        }
        engine::drained(ctx, &[]).await;
    }

    // Before anything is pulled rather than after: a fetch that runs out of disk
    // part-way reads as the update being broken rather than as the disk being full.
    space::admits(ctx).await?;

    let claim = engine::claimed(ctx).await?;
    let run = moved(ctx, manifest, &taking).await;
    engine::released(ctx, claim).await;
    let (backup, edits, applied, halted) = run?;

    Ok(Report {
        state: update::state(&changes, &applied),
        changes,
        in_flight: active,
        confirmed: true,
        backup: Some(backup),
        stack_edits: edits,
        applied,
        halted,
    })
}

/// The changes worth taking, in the order the manifest declares their services.
///
/// A step lemonfiber refuses is left out rather than attempted: the database has
/// already been through a later version, and the older binary opening it is what
/// would damage it. It stays in the report, so the refusal is visible rather than a
/// service that quietly did not move.
fn ordered(manifest: &Manifest, changes: &[Change]) -> Vec<Change> {
    manifest
        .services
        .iter()
        .filter_map(|service| {
            changes
                .iter()
                .find(|change| !change.refused && change.service == service.id)
        })
        .cloned()
        .collect()
}

/// One service to move, and the invocation that moves it.
///
/// Carried together because they are decided at different moments and used at the
/// same one: what would change is worked out before anything is agreed to, and how
/// Compose is told to start it is the stack as it stands on disk right now.
struct Step {
    /// The step being taken.
    change: Change,
    /// The Compose invocation that starts that one service.
    argv: Vec<String>,
}

/// Each change with the invocation that takes it, or why none could be built.
///
/// # Errors
///
/// Returns the [`Problem`] for a stack that cannot be resolved or written.
fn steps(ctx: &Ctx, taking: Vec<Change>) -> Result<Vec<Step>, Box<Problem>> {
    taking
        .into_iter()
        .map(|change| {
            let started = Action::Start(vec![change.service.clone()]);
            let (argv, _) = engine::invocation(ctx, &started)?;
            Ok(Step { change, argv })
        })
        .collect()
}

/// Stop, capture, and move — the three that must happen in that order.
async fn moved(
    ctx: &Ctx,
    manifest: &Manifest,
    taking: &[Step],
) -> Result<
    (
        String,
        Vec<crate::model::StackEdit>,
        Vec<Applied>,
        Option<String>,
    ),
    Box<Problem>,
> {
    let edits = whole(ctx, &Action::Stop(Vec::new())).await?;
    // From here the stack is down, so anything that fails before it is brought back
    // has to say so itself: the operator has no other way to learn it.
    //
    // The safety net, and a precondition rather than an offer: a capture that will
    // not write is a refusal here, so nothing opens its state on a newer binary
    // without something to go back to.
    let archive = backup::run(ctx, None)
        .await
        .map_err(|cause| Box::new(left_down(*cause)))?;
    let (applied, halted) = staged(ctx, manifest, taking).await;
    // Everything the capture took down and this run did not move is still down, so a
    // run that got to the end puts the stack back. One that halted does not: starting
    // the rest after meeting a service that would not come back is the wholesale move
    // taking them one at a time exists to avoid, and what to do next is the
    // operator's, who has the report in front of them.
    //
    // A start that will not run is reported rather than raised. By here every step
    // the run meant to take has been taken and each service answered its own probe,
    // so the update is done and there is nothing to undo — and an error in place of
    // the report is what would have somebody reverse a migration that worked.
    let halted = match halted {
        stopped @ Some(_) => stopped,
        None => whole(ctx, &Action::Start(Vec::new()))
            .await
            .err()
            .map(|cause| unrestored(&cause)),
    };
    Ok((archive.path.display().to_string(), edits, applied, halted))
}

/// Run one Compose action over the whole stack, and the operator's own edits it left
/// in place while materialising.
///
/// The exit status is not read. What the stop has to establish is that nothing is
/// writing, which the capture asks the engine rather than a process — so a stop that
/// did not work surfaces as the refusal to capture a live database rather than as a
/// second opinion about the same thing. What the start afterwards has to establish
/// was established service by service for everything this run moved, and the rest is
/// on the versions it was already running.
async fn whole(ctx: &Ctx, action: &Action) -> Result<Vec<crate::model::StackEdit>, Box<Problem>> {
    let (argv, edits) = engine::invocation(ctx, action)?;
    ctx.runner
        .run(&argv)
        .await
        .map_err(|err| Box::new(err.problem()))?;
    Ok(edits)
}

/// Why a run that met a service which would not come back stopped there, and where
/// that leaves the stack.
///
/// Where it leaves the stack is said because it is not where the operator left it:
/// everything came down for the capture and only what moved is back up, which is a
/// fact somebody reading a halted run has no other way to learn.
fn halt(change: &Change) -> String {
    format!(
        "{} did not come back on {}, so the run stopped there and nothing after it was touched. \
         Everything came down for the backup and only what moved is up again — lemonfiber up \
         brings the rest of the stack back",
        change.service, change.target
    )
}

/// Why a run that took every step it meant to has left the stack down anyway.
///
/// Said in the report rather than raised as a problem, because the update is not what
/// failed: every service that was going to move moved, and each answered its own probe
/// before the next was touched. What did not happen is the stack coming back after the
/// capture — a second fact about the same run, and the only one left to act on.
fn unrestored(cause: &Problem) -> String {
    format!(
        "The stack would not start again afterwards: {}. Everything came down for the backup \
         and only what moved is up again — lemonfiber up brings the rest of the stack back. \
         The update itself is done: every service above moved and answered its own probe, so \
         there is nothing here to roll back",
        cause.summary
    )
}

/// What became of each service the run reached, and why it stopped where it did.
async fn staged(ctx: &Ctx, manifest: &Manifest, taking: &[Step]) -> (Vec<Applied>, Option<String>) {
    let mut applied = Vec::new();
    let mut halted = None;
    let mut left = taking.iter();
    for step in left.by_ref() {
        let (ending, detail) = one(ctx, manifest, step).await;
        applied.push(Applied::ended(&step.change, ending, detail));
        if ending != Ending::Updated {
            halted = Some(halt(&step.change));
            break;
        }
    }
    // Everything the run never got to, said rather than left out. A service missing
    // from the list would read as one that was already where it should be.
    for step in left {
        applied.push(Applied::ended(&step.change, Ending::NotReached, None));
    }
    (applied, halted)
}

/// Move one service onto its pin, and find out whether it came back.
async fn one(ctx: &Ctx, manifest: &Manifest, step: &Step) -> (Ending, Option<String>) {
    match ctx.runner.run(&step.argv).await {
        Err(failure) => (Ending::NotFetched, Some(failure.to_string())),
        Ok(output) if !output.succeeded() => (Ending::NotFetched, Some(refusal(&output))),
        Ok(_) => answering(ctx, manifest, &step.change.service).await,
    }
}

/// What Compose said when it would not bring a service up.
///
/// Its own words, whichever stream they arrived on: a pull that could not reach the
/// registry writes to one and a compose file it will not accept writes to the other,
/// and an operator needs whichever of them there is.
fn refusal(output: &crate::ports::process::Output) -> String {
    let said = output.stderr.trim();
    if said.is_empty() {
        return output.stdout.trim().to_owned();
    }
    said.to_owned()
}

/// Wait for one service to be answering on its new image, or say why it is not.
///
/// One service rather than the whole plan, which is what makes this a staged update
/// at all: the rest of the stack is deliberately still down, and a wait that counted
/// it would time out on the services this run has not reached yet.
async fn answering(ctx: &Ctx, manifest: &Manifest, service: &str) -> (Ending, Option<String>) {
    let profiles: Vec<String> = manifest
        .services
        .iter()
        .filter(|one| one.id == service)
        .map(|one| one.profile.clone())
        .collect();
    let deadline = ctx.clock.now() + ctx.patience;
    loop {
        let Ok(containers) = ctx.engine.list(&ctx.settings.project).await else {
            return (
                Ending::NotStarted,
                Some("the container engine stopped answering".to_owned()),
            );
        };
        let seen = survey(manifest, &profiles, &containers);
        let state = seen
            .iter()
            .find(|one| one.id == service)
            .map(|one| one.state);
        match state {
            Some(state) if state.settled() && !state.wants_attention() => {
                return (Ending::Updated, None)
            }
            Some(state) if state.settled() => {
                return (
                    Ending::NotStarted,
                    Some("it started on the new image and is not working".to_owned()),
                )
            }
            _ => {}
        }
        if ctx.clock.now() >= deadline {
            return (
                Ending::NotStarted,
                Some("it did not finish starting".to_owned()),
            );
        }
        tokio::time::sleep(POLL).await;
    }
}
