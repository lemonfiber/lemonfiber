//! The reads that arrive over time rather than all at once.
//!
//! Logs, a pull and a watch each produce output as they go, so none of them is a
//! value that comes back from dispatch — they stream, and something has to sit
//! with them until they end. That is what is here, kept out of the dispatcher so
//! it stays a dispatcher.
//!
//! Starting and stopping sit here too, with the announcement each of them makes
//! first. They are the same kind of thing — Compose narrates for minutes and its
//! report comes at the end — and they came across from `main` when the length rule
//! sent that file looking for a module to put a concern in. This is the module that
//! already held the half they call.

use std::process::ExitCode;

use lemonfiber_core::app::{
    claimed, dispatch, in_flight, logs, pull_progress, released, start_progress, started, Command,
    Ctx, Outcome, Waiting,
};
use lemonfiber_core::model::kind::{self, Kind};
use lemonfiber_core::model::Envelope;
use lemonfiber_core::ports::docker::LogQuery;
use lemonfiber_core::ports::process::Progress as PullEvent;
use lemonfiber_core::ports::Narrator;
use lemonfiber_core::stack::compose::Action;

use crate::exit::{complain, settled, FAILURE};
use crate::keyboard::{Console, Keyboard};
use crate::prompt::Answers as _;
use crate::render::downloads::interrupting;
use crate::render::stack::Doing;
use crate::render::{logged, render, UNRENDERABLE};
use crate::say::{complain, emit, say};
use crate::setup::Surface as _;
use crate::stopping::{answered, asking, Asking, Choice, ASK_TO_WAIT};

/// Print log lines as they arrive, until the stream ends.
///
/// Machine-readable output is one envelope per line rather than one document
/// containing all of them: a stream has no last element to close a document
/// with, and a consumer of `--follow --json` needs each line when it happens
/// rather than when the service stops.
pub(crate) async fn stream(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    follow: bool,
    tail: u32,
    json: bool,
) -> ExitCode {
    let query = LogQuery { tail, follow };
    let mut lines = match logs(ctx, forms, services, query).await {
        Ok(opened) => opened,
        Err(problem) => return complain(&problem),
    };
    let mut seen = 0_u64;
    while let Some(line) = lines.recv().await {
        seen += 1;
        if json {
            emit!(
                "{}",
                Envelope::new(kind::LOG, &line)
                    .to_json()
                    .unwrap_or(UNRENDERABLE.to_owned())
            );
        } else {
            say!("{}", logged(&line.service, &line.line));
        }
    }

    // Silence and "no output" are different answers, and a viewer that renders
    // them identically leaves the operator wondering which one they got.
    if seen == 0 && !json {
        say!("no output");
    }
    ExitCode::SUCCESS
}

/// Pull the images the forms need, showing each as it downloads.
///
/// Image pulls are gigabytes and take minutes; a silent wait reads as a hang, so
/// the operator is told the wait is coming and then shown Compose's per-image
/// progress as it arrives.
pub(crate) async fn pull(ctx: &Ctx, forms: &[String], json: bool) -> ExitCode {
    // Claimed here rather than inside `pull_showing`, which setup also calls: a pull
    // asked for on its own is a lifecycle operation like every other, and the same
    // request through the dispatcher already claims. Setup's pull is left alone —
    // it is followed immediately by a start that claims, and a first run has nothing
    // to race. Given back on both paths out.
    //
    // Recorded under the action's own word rather than one written here, so a run
    // waiting behind a streamed pull is told the same thing as one waiting behind the
    // pull the dispatcher runs. They are the same operation to whoever is waiting.
    let claim = match claimed(ctx, Action::Pull.name()).await {
        Ok(claim) => claim,
        Err(problem) => return complain(&problem),
    };
    let code = match pull_showing(ctx, forms, json).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    };
    released(ctx, claim).await;
    code
}

/// Stream a pull to the screen, or report why it could not run.
///
/// Split from [`pull`] so setup's Start phase can pull-with-progress and, only if
/// that succeeds, go on to bring the stack up — a `Result` it can branch on rather
/// than an exit code it cannot compare.
pub(crate) async fn pull_showing(ctx: &Ctx, forms: &[String], json: bool) -> Result<(), ExitCode> {
    // A rehearsal reports what would be pulled rather than pulling it; the buffered
    // path already renders that, so a dry run borrows it rather than streaming.
    if ctx.dry_run {
        return pull_rehearsal(ctx, forms, json).await;
    }

    // The expected-duration statement, before the wait — qualitative, because the
    // real figure depends on the operator's line and what is already cached.
    if !json {
        say!("Pulling images — usually a few minutes, and several gigabytes.");
    }

    let mut progress = pull_progress(ctx, forms)
        .await
        .map_err(|problem| complain(&problem))?;
    let mut failed = false;
    while let Some(event) = progress.recv().await {
        match event {
            PullEvent::Line(line) => emit_line(kind::PULL, &line, json),
            // A non-zero exit is the pull's own report that an image did not come
            // down; the operator is told, and a script sees a non-zero code.
            PullEvent::Ended(status) => failed = status != Some(0),
        }
    }

    if failed {
        if !json {
            complain!("\nSome images could not be pulled — check the output above.");
        }
        return Err(ExitCode::from(FAILURE));
    }
    Ok(())
}

/// A dry run's rehearsal: report what a pull would fetch without fetching it,
/// borrowing the buffered path's rendering rather than streaming a pull that will
/// not happen.
pub(crate) async fn pull_rehearsal(
    ctx: &Ctx,
    forms: &[String],
    json: bool,
) -> Result<(), ExitCode> {
    match dispatch(
        Command::Pull {
            forms: forms.to_vec(),
        },
        ctx,
    )
    .await
    {
        Ok(outcome) => {
            render(&outcome, json);
            Ok(())
        }
        Err(problem) => Err(complain(&problem)),
    }
}

/// Render one line a narrated command wrote — wrapped in an envelope under `--json`,
/// indented beneath the command for a person to read.
///
/// The kind is the caller's, because a consumer filtering a `--json` stream is
/// filtering on it: a pull's lines and a start's are different things happening.
pub(crate) fn emit_line(kind: Kind, line: &str, json: bool) {
    if !json {
        say!("  {line}");
        return;
    }
    emit!(
        "{}",
        Envelope::new(kind, line)
            .to_json()
            .unwrap_or(UNRENDERABLE.to_owned())
    );
}

/// What a wait says, put where the command's own narration is already going.
///
/// The other half of the seam a streamed start has. Compose's lines arrive on a
/// channel and go out through [`emit_line`]; the wait that follows them arrives
/// through this and goes out the same way, under the same kind — a consumer
/// filtering `--json` for a start is filtering for the whole of one, and the wait
/// is the part of a start it can least afford to miss.
///
/// Who the output is for is asked rather than carried, for the reason the funnel
/// settles it once: a flag threaded to every place that might eventually print is a
/// flag that will one day be threaded wrong, and this one is reached from a context
/// built before the command is known.
pub(crate) struct Narrating;

#[async_trait::async_trait]
impl Narrator for Narrating {
    async fn say(&self, said: &str) {
        emit_line(kind::START, said, crate::say::for_a_parser());
    }
}

/// Report what stopping would interrupt, and say what the operator decided about it.
///
/// Silent where nothing is coming down, which is the ordinary case — a teardown that
/// remarked on an empty queue every time would teach the operator to skip the part
/// that matters on the day it is not empty.
///
/// The question is put here and the waiting is not: an answer is what the teardown
/// carries, and a surface that sat in the loop itself would be a wait only the
/// command line could ask for.
pub(crate) async fn settle(ctx: &Ctx, forms: &[String], wait: bool, yes: bool) -> Choice {
    let active = in_flight(ctx, forms).await;
    if active.is_empty() {
        return Choice::Stop;
    }
    interrupting(&active).print();

    match asking(wait, yes, Console.interactive()) {
        Asking::Settled(choice) => choice,
        Asking::Ask => answered(&Keyboard.ask(ASK_TO_WAIT)),
    }
}

/// Start the forms, showing what Compose says as it says it.
///
/// A start is minutes of silence otherwise, and the silence is the problem: pulling
/// an image, creating a network and waiting on a health check all look identical from
/// outside, and the one that has hung looks identical to all three. So Compose's own
/// narration goes to the screen as it arrives, and the report follows it.
pub(crate) async fn start(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    json: bool,
) -> ExitCode {
    // A streamed start does not go through the dispatcher, so it claims the stack
    // here rather than inheriting the claim `lifecycle` takes. Given back below on
    // both paths out, for the same reason it is given back there.
    //
    // Recorded under the action's own word for the reason a streamed pull is: a
    // stack held by a start is held by a start, and which of the two code paths is
    // running it is not a fact the operator waiting on it has any use for.
    let claim = match claimed(ctx, Action::Up.name()).await {
        Ok(claim) => claim,
        Err(problem) => return complain(&problem),
    };

    // A rehearsal runs nothing, so it has nothing to narrate — and it needs no
    // separate path, because the report it wants is the one built from the same plan
    // without the single irreversible step, which is what a status of nothing gets.
    let status = if ctx.dry_run {
        None
    } else {
        match narrated(ctx, forms, services, json).await {
            Ok(status) => status,
            Err(code) => {
                released(ctx, claim).await;
                return code;
            }
        }
    };

    // The report is asked for whatever the status was: a start that failed part way
    // has still started something, and which services came up is the first thing an
    // operator needs in order to do anything about the ones that did not.
    let outcome = started(ctx, forms, services, status).await;
    released(ctx, claim).await;
    match outcome {
        Ok(report) => {
            let outcome = Outcome::Lifecycle(report);
            render(&outcome, json);
            settled(&outcome)
        }
        Err(problem) => complain(&problem),
    }
}

/// Spawn the start and put Compose's narration on the screen as it arrives.
///
/// Gives back the exit status Compose ended on, or the code a surface should exit
/// with where the command could not be spawned at all.
async fn narrated(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    json: bool,
) -> Result<Option<i32>, ExitCode> {
    let mut progress = start_progress(ctx, forms, services)
        .await
        .map_err(|problem| complain(&problem))?;
    let mut status = None;
    while let Some(event) = progress.recv().await {
        match event {
            PullEvent::Line(line) => emit_line(kind::START, &line, json),
            PullEvent::Ended(code) => status = code,
        }
    }
    Ok(status)
}

/// Announce what starting will affect, then start it, narrated as it goes.
///
/// Starting does not go through dispatch, for the same reason a pull and a watch do
/// not: Compose narrates for minutes and the report comes at the end, which is not a
/// value that arrives once.
pub(crate) async fn starting(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    json: bool,
) -> ExitCode {
    // Not announced where services are named. The announcement is about what a form
    // holds, and saying "starts eight services" before starting two of them would be
    // a sentence about a set the operator did not ask for.
    if services.is_empty() {
        announce(ctx, forms, json, Doing::Starting).await;
    }
    start(ctx, forms, services, json).await
}

/// Announce what stopping would affect, put the question about anything still coming
/// down, and hand the answer to the teardown.
///
/// Both happen before the teardown rather than during it: an operator who is going to
/// be told a download is at ninety per cent wants to be told while stopping is still
/// a question, not while it is already happening.
pub(crate) async fn halting(
    ctx: &Ctx,
    forms: Vec<String>,
    services: Vec<String>,
    wait: bool,
    yes: bool,
    json: bool,
) -> Command {
    // Stopping named services and tearing a form down are different requests rather
    // than one request with an argument, and Compose spells them differently too.
    // The command line refuses the two flags together for the same reason.
    if !services.is_empty() {
        return Command::Halt { forms, services };
    }
    announce(ctx, &forms, json, Doing::Stopping).await;
    // Asked only where there is somebody to ask. A machine-readable run is put no
    // prompt — it has nobody to answer one, and a report not in the envelope is noise
    // on a stream something is parsing — so what it typed is what the teardown gets.
    let waiting = if json {
        wait
    } else {
        settle(ctx, &forms, wait, yes).await == Choice::Wait
    };
    Command::Down {
        forms,
        wait: Waiting::from(waiting),
    }
}

async fn announce(ctx: &Ctx, forms: &[String], json: bool, doing: Doing) {
    if json {
        return;
    }
    if let Ok(Outcome::Preview(plan)) = dispatch(
        Command::Preview {
            forms: forms.to_vec(),
        },
        ctx,
    )
    .await
    {
        crate::render::stack::affects(&plan, doing).print();
    }
}

#[cfg(test)]
mod tests;
