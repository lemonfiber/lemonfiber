//! Starting, pulling, stopping and following logs, with what each says as it goes.
//!
//! Each is dispatched; what the engine writes on the way reaches the screen through
//! the narrator the context carries, and the report follows.

use std::process::ExitCode;

use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, ReadingLogs, Waiting};
use lemonfiber_core::logs::Line;
use lemonfiber_core::model::kind::{self, Kind};
use lemonfiber_core::model::Envelope;
use lemonfiber_core::ports::docker::LogLine;
use lemonfiber_core::ports::Narrator;

use lemonfiber::cli::{RawDown, RawSwitch};

use crate::exit::{complain, settled, FAILURE};
use crate::installing::Agreed;
use crate::keyboard::{Console, Keyboard};
use crate::prompt::Answers as _;
use crate::render::downloads::interrupting;
use crate::render::stack::Doing;
use crate::render::{logged, render, UNRENDERABLE};
use crate::say::{complain, emit, say};
use crate::setup::Surface as _;
use crate::stopping::{answered, asking, Asking, Choice, ASK_TO_WAIT};

/// Read the logs through the dispatcher, each line printed through the narrator as it
/// arrives, until the stream ends.
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
    let asked = Command::Logs(ReadingLogs {
        forms: forms.to_vec(),
        services: services.to_vec(),
        tail,
        follow,
    });
    match dispatch(asked, ctx).await {
        Ok(Outcome::Logs(read)) if read.lines == 0 && !json => {
            say!("no output");
            ExitCode::SUCCESS
        }
        Ok(_) => ExitCode::SUCCESS,
        Err(problem) => complain(&problem),
    }
}

/// Pull the images the forms need, showing each as it downloads.
pub(crate) async fn pull(ctx: &Ctx, forms: &[String], json: bool) -> ExitCode {
    match pull_showing(ctx, forms, json).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

/// What `command` came to, or the exit its refusal was said with.
pub(crate) async fn dispatched(command: Command, ctx: &Ctx) -> Result<Outcome, ExitCode> {
    match dispatch(command, ctx).await {
        Ok(outcome) => Ok(outcome),
        Err(problem) => Err(complain(&problem)),
    }
}

/// Pull the images the forms need through the dispatcher, each line the engine writes
/// shown as it arrives through the narrator, or report why it could not run.
///
/// A rehearsal pulls nothing and renders what would be pulled.
pub(crate) async fn pull_showing(ctx: &Ctx, forms: &[String], json: bool) -> Result<(), ExitCode> {
    if !json && !ctx.dry_run {
        say!("Pulling images — usually a few minutes, and several gigabytes.");
    }
    let pulled = dispatched(
        Command::Pull {
            forms: forms.to_vec(),
        },
        ctx,
    )
    .await?;
    if ctx.dry_run {
        render(&pulled, json);
        return Ok(());
    }
    if matches!(&pulled, Outcome::Lifecycle(report) if report.status == Some(0)) {
        return Ok(());
    }
    if !json {
        complain!("\nSome images could not be pulled — check the output above.");
    }
    Err(ExitCode::from(FAILURE))
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

    async fn pulled(&self, line: &str) {
        emit_line(kind::PULL, line, crate::say::for_a_parser());
    }

    async fn logged(&self, line: &LogLine) {
        if crate::say::for_a_parser() {
            emit!(
                "{}",
                Envelope::new(kind::LOG, &Line::from(line.clone()))
                    .to_json()
                    .unwrap_or(UNRENDERABLE.to_owned())
            );
        } else {
            say!("{}", logged(&line.service, &line.line));
        }
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
pub(crate) async fn settle(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    wait: bool,
    yes: bool,
) -> Choice {
    let asked = Command::InFlight {
        forms: forms.to_vec(),
        services: services.to_vec(),
    };
    let active = match dispatch(asked, ctx).await {
        Ok(Outcome::InFlight(report)) => report.downloads,
        Ok(_) | Err(_) => Vec::new(),
    };
    if active.is_empty() {
        return Choice::Stop;
    }
    interrupting(&active).print();

    match asking(wait, yes, Console.interactive()) {
        Asking::Settled(choice) => choice,
        Asking::Ask => answered(&Keyboard.ask(ASK_TO_WAIT)),
    }
}

/// Start the forms, or the services named, through the dispatcher: what the engine
/// pulls on the way is shown as it arrives through the narrator, and the report follows.
pub(crate) async fn start(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    offer: Option<&str>,
    json: bool,
) -> ExitCode {
    let command = if services.is_empty() {
        Command::Up {
            forms: forms.to_vec(),
            offer: offer.map(str::to_owned),
        }
    } else {
        Command::Start {
            forms: forms.to_vec(),
            services: services.to_vec(),
        }
    };
    match dispatch(command, ctx).await {
        Ok(outcome) => {
            render(&outcome, json);
            settled(&outcome)
        }
        Err(problem) => complain(&problem),
    }
}

/// Announce what starting will affect, then start it, narrated as it goes.
pub(crate) async fn starting(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    offer: Option<String>,
    json: bool,
) -> ExitCode {
    // Not announced where services are named. The announcement is about what a form
    // holds, and saying "starts eight services" before starting two of them would be
    // a sentence about a set the operator did not ask for.
    let mut answered = None;
    if services.is_empty() {
        let unanswered = Command::Up {
            forms: forms.to_vec(),
            offer: None,
        };
        answered = match answered_installs(ctx, forms, offer, json, unanswered).await {
            Ok(answered) => answered,
            Err(code) => return code,
        };
        announce(ctx, forms, json, Doing::Starting).await;
    }
    start(ctx, forms, services, answered.as_deref(), json).await
}

/// Announce what stopping would affect, put the question about anything still coming
/// down, and hand the answer to the teardown.
///
/// Both happen before the teardown rather than during it: an operator who is going to
/// be told a download is at ninety per cent wants to be told while stopping is still
/// a question, not while it is already happening.
pub(crate) async fn halting(ctx: &Ctx, down: RawDown, json: bool) -> Command {
    let RawDown {
        forms,
        services,
        wait,
        yes,
    } = down;
    if services.is_empty() {
        announce(ctx, &forms, json, Doing::Stopping).await;
    }
    // Asked only where there is somebody to ask. A machine-readable run is put no
    // prompt — it has nobody to answer one, and a report not in the envelope is noise
    // on a stream something is parsing — so what it typed is what the stop gets.
    let waiting = if json {
        wait
    } else {
        settle(ctx, &forms, &services, wait, yes).await == Choice::Wait
    };
    Command::stopping(forms, services, Waiting::from(waiting))
}

/// The switch to `forms`, answering what it offers to install with the offer typed or the
/// one the operator agrees to at a terminal.
///
/// # Errors
///
/// The code to exit with where an install is on offer and nobody agreed to it, once the
/// report saying so is printed, or where the forms cannot be resolved.
pub(crate) async fn switching(
    ctx: &Ctx,
    asked: RawSwitch,
    json: bool,
) -> Result<Command, ExitCode> {
    let RawSwitch { forms, offer } = asked;
    let unanswered = Command::Switch {
        forms: forms.clone(),
        offer: None,
    };
    let offer = answered_installs(ctx, &forms, offer, json, unanswered).await?;
    Ok(Command::Switch { forms, offer })
}

/// The offer a start of `forms` answers: the one typed, or the one the operator agrees to
/// at a terminal, or none where nothing is on offer. Where nobody agrees, `unanswered`
/// is what is run instead: the start answering nothing, which says what is on offer.
///
/// # Errors
///
/// The code to exit with where an install is on offer and nobody agreed to it, once the
/// report saying so is printed, or where the forms cannot be resolved.
async fn answered_installs(
    ctx: &Ctx,
    forms: &[String],
    offer: Option<String>,
    json: bool,
    unanswered: Command,
) -> Result<Option<String>, ExitCode> {
    if offer.is_some() || ctx.dry_run {
        return Ok(offer);
    }
    let interactive = !json && Console.interactive();
    match crate::installing::agreed(ctx, forms, interactive, &|asked| Keyboard.ask(asked)).await {
        Ok(Agreed::Nothing) => Ok(None),
        Ok(Agreed::Yes(offer)) => Ok(Some(offer)),
        Ok(Agreed::No) => Err(unstarted(ctx, unanswered, json).await),
        Err(problem) => Err(complain(&problem)),
    }
}

/// A start answering nothing, which writes and starts nothing and says what is on offer.
async fn unstarted(ctx: &Ctx, unanswered: Command, json: bool) -> ExitCode {
    match dispatch(unanswered, ctx).await {
        Ok(outcome) => {
            render(&outcome, json);
            settled(&outcome)
        }
        Err(problem) => complain(&problem),
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
