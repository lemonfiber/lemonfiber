//! Putting right what the diagnosis found, at the operator's word.
//!
//! The word is the whole of it. `doctor` looks and changes nothing; `doctor --fix` offers
//! each repair with what it would do and what else changes, and waits to be told; and only
//! `--yes` on the same run carries them out unasked, which is a thing somebody types
//! deliberately rather than a default anybody can fall into.
//!
//! What may be offered, what has been declined and what has been tried too often are all
//! the core's to decide. What is here is the asking.

use std::process::ExitCode;

use lemonfiber_core::app::repair::{mend, putting_right, retracting, Confirm, Consent};
use lemonfiber_core::app::{Ctx, Outcome};
use lemonfiber_core::repair::{Repair, Stance};

use lemonfiber_core::config::paths::Paths;

use crate::exit::USAGE;
use crate::prompt::{yes_no, Answers};
use crate::render::Lines;
use crate::say::complain;
use lemonfiber::cli::Mending;

/// Offer the repairs and carry out the ones agreed to, or put back what the last one did.
pub(crate) async fn run(
    ctx: &Ctx,
    paths: Paths,
    asked: Mending,
    answers: &(dyn Answers + Sync),
    json: bool,
) -> ExitCode {
    if asked.undo {
        return undone(ctx, &paths, json).await;
    }
    // Nobody is there to answer a prompt in machine-readable mode, and a script that wanted
    // repairs carried out says so with --yes. So one that did not gets the offer and no
    // action, which is what report-only is for.
    //
    // A run with nowhere to read an answer from is refused rather than asked. The offer
    // reaches a terminal nobody is at, and the read that follows it blocks on input that
    // never comes — once per repair, invisibly, with the run appearing to hang.
    //
    // Two of the three consents are settled before the run begins and are data, so
    // they go in through the entry a browser goes in through. Nothing here decides
    // what each one comes to.
    let consent = match (asked.fixing.yes, json, answers.present()) {
        (true, _, _) => Some(Consent::Standing),
        (false, true, _) => Some(Consent::Offer),
        // The third is a question put mid-run and answered by whoever is at the
        // terminal, which is the one shape no request can carry.
        (false, false, true) => None,
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

    let repaired = match &consent {
        Some(consent) => putting_right(ctx, consent, asked.fixing.disruptive).await,
        None => mend(ctx, Stance::Ask, asked.fixing.disruptive, &Asking(answers)).await,
    };
    match repaired {
        Ok(report) => answered(&Outcome::Repair(report), json),
        Err(problem) => crate::complain(&problem),
    }
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
///
/// The deciding and the doing are the core's — which repair was last, what reversing it
/// takes, which of those need a service to reach, and whether this run may act at all.
/// What is here is the saying. This path does not go through the dispatcher, so the
/// rehearsal verdict is taken inside the call rather than above it; reading the flag
/// here would be a second reading to keep in step with the one that already exists.
async fn undone(ctx: &Ctx, paths: &Paths, json: bool) -> ExitCode {
    match retracting(ctx, paths).await {
        Ok(reversal) => answered(&Outcome::Undo(reversal), json),
        Err(problem) => crate::complain(&problem),
    }
}

/// Asking whoever is at the terminal.
struct Asking<'a>(&'a (dyn Answers + Sync));

impl Confirm for Asking<'_> {
    /// Ask about one repair, having said what it would do and what else changes.
    ///
    /// Stated before the question rather than after it, because an effect somebody learns
    /// about afterwards is not something they agreed to.
    fn agreed(&self, repair: &Repair) -> bool {
        stated(repair).print();
        yes_no(self.0, &format!("{}?", repair.does), false)
    }
}

/// What is about to be agreed to, built as lines like every other answer this binary
/// gives rather than printed where it is decided — the terminal is reached in one place.
fn stated(repair: &Repair) -> Lines {
    let mut lines = Lines::default();
    for effect in &repair.effects {
        lines.put(format!("  {effect}"));
    }
    if !repair.reversible {
        lines.put("  This one cannot be undone.");
    }
    lines
}

#[cfg(test)]
mod tests;
