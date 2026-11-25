//! Putting right what the diagnosis found, at the operator's word.
//!
//! The word is the whole of it. `doctor` looks and changes nothing; `doctor --fix` offers
//! each repair with what it would do and what else changes, and waits to be told; and only
//! `--yes` on the same run carries them out unasked, which is a thing somebody types
//! deliberately rather than a default anybody can fall into.
//!
//! What may be offered, what has been declined and what has been tried too often, and the
//! asking itself, are the core's. What is here is which consent the run carries.

use std::process::ExitCode;
use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::ports::asking::Asking;
use lemonfiber_core::repair::run::Consent;
use lemonfiber_error::Problem;

use crate::exit::USAGE;
use crate::say::complain;
use lemonfiber::cli::Mending;

/// Offer the repairs and carry out the ones agreed to, or put back what the last one did.
pub(crate) async fn run(
    ctx: &Ctx,
    asked: Mending,
    answers: Arc<dyn Asking>,
    json: bool,
) -> ExitCode {
    if asked.undo {
        return undone(ctx, json).await;
    }
    let reporting = json || ctx.dry_run;
    let consent = match (asked.fixing.yes, reporting, answers.present()) {
        (true, _, _) => Consent::Standing,
        (false, true, _) => Consent::Offer,
        (false, false, true) => Consent::Asked,
        (false, false, false) => {
            complain!(
                "error: repairing here is non-interactive, so there is nobody to agree to each repair:"
            );
            complain!("  --yes    carry out every repair offered, without asking");
            complain!("  --json   report what would be repaired and change nothing");
            complain!("\nRun it in a terminal to be asked about each instead.");
            return ExitCode::from(USAGE);
        }
    };

    match repaired(ctx, consent, asked.fixing.disruptive, answers).await {
        Ok(outcome) => answered(&outcome, json),
        Err(problem) => crate::complain(&problem),
    }
}

/// What a forward repair run comes to, each repair an unsettled consent offers put to
/// whoever `answers` reaches.
async fn repaired(
    ctx: &Ctx,
    consent: Consent,
    disruptive: bool,
    answers: Arc<dyn Asking>,
) -> Result<Outcome, Box<Problem>> {
    let mut asking = ctx.clone();
    asking.asking = answers;
    dispatch(
        Command::Repair {
            consent,
            disruptive,
        },
        &asking,
    )
    .await
}

/// One outcome, read out and scored, through the paths every other answer takes.
///
/// The offer and the question in front of it are this surface's own; what became of
/// the run is not. Rendered where every outcome is rendered and scored where every
/// outcome is scored, so the words a person reads and the document a script parses
/// are the ones the web serves for the same run.
fn answered(outcome: &Outcome, json: bool) -> ExitCode {
    let code = crate::exit::settled(outcome);
    crate::render::render(outcome, json);
    code
}

/// Put back what the last repair changed, or say what putting it back would do.
async fn undone(ctx: &Ctx, json: bool) -> ExitCode {
    match dispatch(Command::Undo { run: None }, ctx).await {
        Ok(outcome) => answered(&outcome, json),
        Err(problem) => crate::complain(&problem),
    }
}

#[cfg(test)]
mod tests;
