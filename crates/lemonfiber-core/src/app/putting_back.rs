//! Putting back one run of changes, named by the stamp the history shows.
//!
//! Between `doctor --undo`, which reverses the last repair and nothing else, and
//! `rewind`, which unwinds the whole journal, sat the thing an operator actually asks
//! for: *that* seed, *that* reconfigure — the one they can see in the history and now
//! regret. This is that middle, and it is the same executor underneath: the changes that
//! live inside a service go back through that service, and what is left goes back on the
//! host.
//!
//! A run, never a change on its own. The operation and the stamp together are the unit
//! an operator agreed to, and undoing half of one leaves a machine in a state nobody
//! chose — so naming any change of a run names the run, which is what the history's
//! "alongside" count tells them before they ask.
//!
//! Nothing is put back until every change of the run has been judged. A reversal that
//! carried out three of five and then met one it could not is a machine in a state
//! nobody has been told about, and the judgement is already available without touching
//! anything.

use crate::error::{Diagnose as _, Problem, Remedy, Severity, State};
use crate::journal::{Change, Undo};
use crate::rollback::{standing, together, Reversal as Judgement};

use super::repair::told;
use super::Ctx;

/// What putting a run back came to.
///
/// A report rather than a bare list, because it is what an envelope carries and an
/// envelope carries a document. Two lists, and the second is the one that matters when
/// it is not empty: what went back, and what did not with the reason it did not.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, schemars::JsonSchema)]
#[schemars(rename = "UndoReversal")]
pub struct Reversal {
    /// What was put back, in the order it was — or, on a run that only said what it
    /// would do, what would go back.
    pub reversed: Vec<Undo>,
    /// What was not put back, each with the reason it was not.
    ///
    /// A reversal an operator asked for by name has to say what it did *not* do. Five
    /// changes asked back and three carried out is a machine in a state nobody has been
    /// told about, and "some of it worked" is the sentence that makes somebody go
    /// looking by hand. Empty where everything went back, which is the common case.
    ///
    /// On a run that only said what it would do, this is what it cannot promise: a
    /// change that goes back through the service that made it goes back only where that
    /// service is answering, and a rehearsal has not asked one.
    pub left: Vec<Left>,
    /// What putting these changes back means beyond the changes themselves.
    ///
    /// Empty on almost every run. What lands here is a change the judgement can put
    /// back in full and that still leaves something behind — the one in force today
    /// being a setting that re-points where data lives, which goes back while the
    /// library stays exactly where it was moved to.
    ///
    /// Neither list above can carry it. It did not fail to go back, so it is not what
    /// was left; and reporting only that it went back would send an operator looking
    /// for their files at an address that no longer names them.
    #[serde(default)]
    pub noted: Vec<Noted>,
    /// Whether this run only said what it would put back.
    ///
    /// A flag rather than a second shape, because the two lists mean the same thing
    /// either way and a caller reading them should read one document. What changes is
    /// the tense a surface says them in.
    pub rehearsed: bool,
}

/// One change a reversal did not put back, and why it did not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[schemars(rename = "UndoLeft")]
pub struct Left {
    /// What the change was against — a service, or lemonfiber's own environment file.
    pub target: String,
    /// Why it is still standing, in the operator's terms.
    pub because: String,
}

/// What putting one change back means beyond the change itself.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[schemars(rename = "UndoNoted")]
pub struct Noted {
    /// What the change was against.
    pub target: String,
    /// What goes back, what does not go with it, and what to do instead.
    pub because: String,
}

pub(crate) use crate::error::codes::undo::NO_SUCH_RUN;

pub(crate) use crate::error::codes::undo::MORE_THAN_ONE_RUN;

pub(crate) use crate::error::codes::undo::CANNOT_SUCCEED;

pub(crate) use crate::error::codes::undo::NOWHERE_TO_LOOK;

/// The operation a reversal records its own work under, so it can be put back in turn.
pub const OPERATION: &str = "undo";

/// Why a rehearsal will not promise a change that lives inside a service.
const NEEDS_THE_SERVICE: &str = "it goes back through the service that made it, so it \
     goes back only where that service is answering when this is run for real";

/// Put back the last repair, or the run a stamp names.
///
/// # Errors
///
/// Returns a [`Problem`] where the run cannot be found, where the stamp names more than
/// one, where the judgement says the run cannot be put back, or for any reason the
/// executor underneath gives.
pub async fn undo(ctx: &Ctx, run: Option<String>) -> Result<super::Outcome, Box<Problem>> {
    reversing(ctx, run.as_deref())
        .await
        .map(super::Outcome::Undo)
}

/// Put back the last repair, or the run a stamp names.
///
/// # Errors
///
/// Returns a [`Problem`] where the run cannot be found, where the stamp names more than
/// one, where the judgement says the run cannot be put back, or for any reason the
/// executor underneath gives.
pub async fn reversing(ctx: &Ctx, run: Option<&str>) -> Result<Reversal, Box<Problem>> {
    match run {
        None => super::repair::reversing(ctx).await,
        Some(at) => named(ctx, at).await,
    }
}

/// Put back the run stamped `at`.
async fn named(ctx: &Ctx, at: &str) -> Result<Reversal, Box<Problem>> {
    let paths = super::targets::layout(ctx).ok_or_else(|| Box::new(nowhere_to_look()))?;
    let journal = super::recover::journal_at(&paths.journal());
    let changes = journal.changes();

    let operation = operation_at(changes, at)?;
    let run: Vec<&Change> = together(changes, &operation, at);
    carried_out(ctx, &paths, changes, &run, at).await
}

/// Put back every change one operation ever made.
///
/// The same machinery as an undo of a stamp and deliberately not a second one: what
/// differs between taking a plugin off a machine and putting back a run somebody named
/// is which changes are gathered, and nothing else. The judgement, the order, the
/// execution, the record of having done it and the account given are one path — so a
/// removal inherits the rollback layer's refusals rather than being written to agree
/// with them.
///
/// # Errors
///
/// Where there is nowhere to look for the record, where the judgement says a change
/// cannot be put back — drift, or a later change that depends on it — or for any reason
/// the executor underneath gives.
pub(crate) async fn everything(ctx: &Ctx, operation: &str) -> Result<Reversal, Box<Problem>> {
    let paths = super::targets::layout(ctx).ok_or_else(|| Box::new(nowhere_to_look()))?;
    let journal = super::recover::journal_at(&paths.journal());
    let changes = journal.changes();

    let run = crate::rollback::everything(changes, operation);
    carried_out(ctx, &paths, changes, &run, operation).await
}

/// Whether [`everything`] would go ahead, asked without touching anything.
///
/// For a caller with something of its own to do before the reversal — a plugin's
/// containers come off before its files go back — and which must not do it if the
/// reversal is then going to refuse. Asked of the same judgement [`everything`] makes,
/// so the answer here and the refusal there cannot disagree.
///
/// # Errors
///
/// The ones [`everything`] would give before touching anything: nowhere to look for
/// the record, or a change the judgement will not put back.
pub(crate) fn admitted(ctx: &Ctx, operation: &str) -> Result<(), Box<Problem>> {
    let paths = super::targets::layout(ctx).ok_or_else(|| Box::new(nowhere_to_look()))?;
    let journal = super::recover::journal_at(&paths.journal());
    let changes = journal.changes();
    judged(
        ctx,
        changes,
        &crate::rollback::everything(changes, operation),
        operation,
    )
    .map(drop)
}

/// Judge a set of changes whole, then put them back newest first.
///
/// `at` is what a refusal calls the thing being put back: the stamp where an operator
/// asked for a run, and the plugin where one is being removed. It is read into the
/// sentence and never looked up by, which is what lets one path serve both.
async fn carried_out(
    ctx: &Ctx,
    paths: &crate::config::paths::Paths,
    changes: &[Change],
    run: &[&Change],
    at: &str,
) -> Result<Reversal, Box<Problem>> {
    let noted = judged(ctx, changes, run, at)?;

    // Newest first, which is the order a reversal has to take and the order the
    // repair's own reversal already takes. A run that made a directory and then made
    // one inside it is put back by removing the inner one first; walking the record
    // forwards would meet the outer directory while its child is still in it, and a
    // directory that will not empty stops the whole reversal.
    let undos: Vec<Undo> = run.iter().rev().map(|change| change.undo()).collect();

    // A rehearsal stops here, and here is where a real run stops being reversible: the
    // judgement above is the whole of what can be known without touching anything, and
    // everything below it reaches a service, rewrites the environment file or records
    // what it did. What it reports is that judgement — every change of the run, split by
    // whether putting it back needs something to be answering.
    if ctx.dry_run {
        return Ok(Reversal {
            noted,
            ..would_reverse(undos)
        });
    }

    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let project = super::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let reached =
        super::recover::reconfigured(ctx, &undos, &manifest.services, project.as_deref()).await;
    let carried = super::recover::carrying_out(&reached.left, &paths.env_file(), Vec::new())?;

    // The account rather than the instruction, which is the division `told` exists to
    // make: what a reversal is carried out *with* holds the values it puts back, and
    // what it reports must not. Applied here as well as on the path that puts back the
    // last repair — both fill in the same `Outcome::Undo`, a terminal prints it and
    // `/api/undo` serves it, and a rule that holds on one of the two roads to it is a
    // rule that holds half the time.
    let reversed: Vec<Undo> = reached
        .put_back
        .iter()
        .chain(carried.done.iter())
        .cloned()
        .map(told)
        .collect();

    // Recorded before the report is built, so a reversal that is reported is a reversal
    // that is in the record. The changes it writes are the inverse of the ones it put
    // back, which is what makes this run answerable to the same command.
    super::recover::journalled(
        &paths.journal(),
        &recording(run, &reversed, &ctx.stamp()),
        ctx.random.as_ref(),
    );

    Ok(Reversal {
        reversed,
        left: standing_after(&reached.unreached, &carried),
        noted,
        rehearsed: false,
    })
}

/// What a run of changes would come to, with none of it coming to that.
///
/// The split is read off each change rather than found out by trying it: a change that
/// lives inside a service goes back through that service and everything else goes back
/// on this machine, which is the division `recover::reconfigured` makes when it carries
/// them out. Finding out the other way would mean a rehearsal opening a client
/// and setting a field in order to discover that it could — which is the write, done to
/// describe itself.
#[must_use]
pub(crate) fn would_reverse(undos: Vec<Undo>) -> Reversal {
    let (through_a_service, here): (Vec<Undo>, Vec<Undo>) = undos
        .into_iter()
        .partition(|undo| matches!(undo.action, crate::journal::Action::Reconfigure { .. }));
    Reversal {
        // Withheld the way a reversal that happened withholds. A rehearsal names the
        // same changes and so would carry the same values out of the journal, and a
        // credential is not less exposed for having been reported about a write nobody
        // made.
        reversed: here.into_iter().map(told).collect(),
        left: through_a_service
            .into_iter()
            .map(|undo| Left {
                target: undo.target,
                because: NEEDS_THE_SERVICE.to_owned(),
            })
            .collect(),
        // Filled in by whoever judged the changes, where anything was judged at all.
        // This function is handed undos rather than changes, and a note is a fact about
        // the change it came from.
        noted: Vec::new(),
        rehearsed: true,
    }
}

/// The one operation a stamp names, refusing where it names none or several.
fn operation_at(changes: &[Change], at: &str) -> Result<String, Box<Problem>> {
    let mut named: Vec<&str> = changes
        .iter()
        .filter(|change| change.at == at)
        .map(|change| change.operation.as_str())
        .collect();
    named.sort_unstable();
    named.dedup();
    match named.as_slice() {
        [] => Err(Box::new(no_such_run(at))),
        [one] => Ok((*one).to_owned()),
        several => Err(Box::new(more_than_one(at, several))),
    }
}

/// The judgement, made whole before anything is touched.
///
/// Answers with what going back *also* means for the changes it will put back, and
/// refuses at the first change it will not: drift, or a later change that depends on
/// it. One function for [`carried_out`] and [`admitted`], so the question asked before
/// a caller's own first step and the one asked before the reversal's are the same one.
///
/// # Errors
///
/// Where a change of the run cannot be put back.
fn judged(
    ctx: &Ctx,
    changes: &[Change],
    run: &[&Change],
    at: &str,
) -> Result<Vec<Noted>, Box<Problem>> {
    // Read once rather than per change: the drift question asks the same file as many
    // times as there are entries otherwise.
    let holds = |key: &str| -> Option<String> {
        let file = ctx.settings.env_file.as_ref()?;
        crate::config::store::read(file)
            .ok()?
            .get(key)
            .map(str::to_owned)
    };

    // What a file holds, for a region: whether it is still the one that was written.
    let reads = |path: &str| -> Option<String> { std::fs::read_to_string(path).ok() };

    // Judged whole before anything is touched. The refusal carries the reason the
    // judgement gave and what to do instead, which for a change nothing here can put
    // back is the only useful half of the answer.
    let mut noted: Vec<Noted> = Vec::new();
    for change in run {
        let position = changes
            .iter()
            .position(|held| held == *change)
            .unwrap_or(changes.len());
        let later = changes.get(position + 1..).unwrap_or_default();
        let verdict = standing(change, later, &holds, &reads);
        if verdict.reversal == Judgement::None {
            // The reason and what to do instead, joined rather than branched on: a
            // refusal carrying no remedy is a shape this judgement does not produce, and
            // a branch for it would be a line no test could ever reach.
            let why = verdict
                .refusal
                .map(|refusal| {
                    [Some(refusal.because), refusal.instead]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<String>>()
                        .join(" — ")
                })
                .unwrap_or_default();
            return Err(Box::new(cannot_succeed(at, &change.target, &why)));
        }
        // It goes back, and going back is not the whole of what happens. A judgement
        // that says so on a change it can still carry out is saying the one thing an
        // operator would otherwise find out by going to look.
        if verdict.reversal == Judgement::Partial {
            if let Some(refusal) = verdict.refusal {
                noted.push(Noted {
                    target: change.target.clone(),
                    because: [Some(refusal.because), refusal.instead]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<String>>()
                        .join(" — "),
                });
            }
        }
    }
    Ok(noted)
}

/// What a reversal left standing, as the report says it.
///
/// Five lists and four reasons, walked as one: a change only a service can undo where
/// that service did not answer, a setting somebody has chosen since, a sealed record that
/// would not open, and a directory still holding something this run did not put there.
/// The middle two cannot arise on this path — the judgement above refuses a run holding
/// either before anything is touched — but they are carried rather than dropped, because
/// the executor can still meet one and a report that silently lost it would be the thing
/// this field exists to prevent.
///
/// The last is the one a plugin removal meets in the ordinary course of things: two
/// plugins keep their documents in one directory, the first of them made it, and the
/// first to leave finds the second's still inside.
fn standing_after(unreached: &[String], carried: &super::recover::Carried) -> Vec<Left> {
    [
        (unreached, "the service that made it did not answer"),
        (
            carried.beyond_reach.as_slice(),
            "the service that made it did not answer",
        ),
        (
            carried.theirs.as_slice(),
            "it holds something chosen since, which putting this back would discard",
        ),
        (
            carried.unread.as_slice(),
            "the record of what it held is sealed under a key this machine no longer has",
        ),
        (
            carried.still_holding.as_slice(),
            "it still holds something this run did not put there",
        ),
    ]
    .into_iter()
    .flat_map(|(targets, because)| {
        targets.iter().map(move |target| Left {
            target: target.clone(),
            because: because.to_owned(),
        })
    })
    .collect()
}

/// The record a reversal keeps of itself, so it can be put back in turn.
///
/// One entry per change actually put back, with the values the other way round: what the
/// original change wrote is what this one found, and what it restored is what stands now.
/// Asking for *this* run then puts the original values back, which is what makes a
/// reversal answerable to the same command as everything else.
///
/// Two of the four kinds record nothing, and the reason is the journal's vocabulary
/// rather than an omission. A path that was made is gone once the reversal removes it,
/// and `Made` would say it had been created; what a service created is never put back at
/// all, so there is nothing to record. Neither has an inverse this record can spell, and
/// inventing one would put a line in the journal that replaying would not reproduce.
fn recording(run: &[&Change], reversed: &[Undo], at: &str) -> Vec<Change> {
    run.iter()
        .filter(|change| reversed.iter().any(|undo| undo.target == change.target))
        .filter_map(|change| {
            inverted(&change.kind).map(|kind| Change {
                at: at.to_owned(),
                operation: OPERATION.to_owned(),
                target: change.target.clone(),
                kind,
            })
        })
        .collect()
}

/// One change with the values it moved between swapped, where that can be said at all.
fn inverted(kind: &crate::journal::Kind) -> Option<crate::journal::Kind> {
    use crate::journal::Kind;
    match kind {
        Kind::Set {
            key,
            previous,
            current,
        } => Some(Kind::Set {
            key: key.clone(),
            previous: Some(current.clone()),
            current: previous.clone().unwrap_or_default(),
        }),
        Kind::Configured {
            resource,
            id,
            field,
            previous,
            current,
        } => Some(Kind::Configured {
            resource: resource.clone(),
            id: id.clone(),
            field: field.clone(),
            previous: Some(current.clone()),
            current: previous.clone().unwrap_or_default(),
        }),
        Kind::Created { .. } | Kind::Made { .. } | Kind::Region { .. } | Kind::Pinned { .. } => {
            None
        }
    }
}

fn nowhere_to_look() -> Problem {
    Problem::new(
        NOWHERE_TO_LOOK,
        Severity::Error,
        "This run has nowhere it knows to look for what was changed",
        "What lemonfiber changed is recorded in its own directory, and this machine \
         would not say where that is. Nothing was put back.",
        Remedy::new("Set a home directory for this user and run it again"),
    )
    .in_state(State::Guided)
}

fn no_such_run(at: &str) -> Problem {
    Problem::new(
        NO_SUCH_RUN,
        Severity::Error,
        format!("Nothing was changed at {at}"),
        format!(
            "No run in the record carries the stamp {at}. It may have fallen outside the \
             horizon the record keeps, or the stamp may be mistyped. Nothing was put back."
        ),
        Remedy::new("Run `lemonfiber history` and take the stamp from the entry you want"),
    )
    .in_state(State::Actionable)
}

fn more_than_one(at: &str, operations: &[&str]) -> Problem {
    Problem::new(
        MORE_THAN_ONE_RUN,
        Severity::Error,
        format!("More than one run is stamped {at}"),
        format!(
            "{at} names {}, and putting back the wrong one is not something to guess at. \
             Nothing was put back.",
            operations.join(" and ")
        ),
        Remedy::new("Ask for one of them by name once the surfaces carry it"),
    )
    .in_state(State::Actionable)
}

fn cannot_succeed(at: &str, target: &str, why: &str) -> Problem {
    Problem::new(
        CANNOT_SUCCEED,
        Severity::Error,
        format!("The run stamped {at} cannot be put back"),
        format!(
            "One of its changes, against {target}, cannot be reversed: {why}. A run goes \
             back whole or not at all, so nothing was put back."
        ),
        Remedy::new("Deal with that change first, or restore from a backup"),
    )
    .in_state(State::Actionable)
}
