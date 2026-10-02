//! What a repairing run writes down about a fault, and what that record decides later.
//!
//! Apart from the running of it because the two answer questions on different timescales.
//! Carrying a repair out is about now: it either worked or it did not, and the second look
//! settles that before the run ends. This is about the runs either side — how often a
//! fault has been seen, how often a fix for it was tried and left it standing, and whether
//! the operator has already said no. None of it can be read off the run that is happening,
//! and all of it decides what the next run offers.

use super::attempts;
use super::attempts::Entry;
use crate::app::{conditions, Ctx};
use crate::condition::Condition;
use crate::repair::{self, Outcome, Repair};

pub(super) use crate::app::conditions::remembered;

use super::Beyond;

/// The faults a repair could answer and has stopped being offered for.
///
/// Only where something could still have been offered: a check nothing can mend was never
/// going to be repaired, and telling its operator that repairs have been exhausted would
/// be describing something that never happened.
pub(super) fn beyond(conditions: &[&Condition], proposals: &[Repair]) -> Vec<Beyond> {
    conditions
        .iter()
        .filter(|condition| condition.is_raised())
        .filter(|condition| repair::exhausted(condition))
        .filter(|condition| {
            proposals
                .iter()
                .any(|repair| repair.check == condition.check)
        })
        .map(|condition| Beyond {
            check: condition.check.clone(),
            remedy: repair::escalation(condition),
        })
        .collect()
}

/// Remember that the operator said no, so it stops being offered until the fault has been
/// away and genuinely come back.
pub(super) fn declined(ctx: &Ctx, repair: &Repair) {
    let mut conditions = conditions::load(ctx);
    conditions.decline(&repair.check);
    conditions::save(ctx, &conditions);
}

/// Record what was done and how it turned out.
///
/// Both halves, always. The count of attempts that left the fault standing, so a repair
/// that is not working stops being offered — and the history, including the attempts that
/// changed nothing, which are the entries somebody reads when the same repair keeps
/// failing to hold a fault down.
pub(super) fn recorded(ctx: &Ctx, repair: &Repair, outcome: &Outcome) {
    let mut conditions = conditions::load(ctx);
    match outcome {
        Outcome::Fixed => conditions.mended(&repair.check),
        // Spent only where the repair ran and the fault is demonstrably still there. One
        // that stopped, was declined, or could not be proved either way has told us nothing
        // about whether lemonfiber is wrong about the cause, which is what the count means.
        Outcome::FixFailed => conditions.attempted(&repair.check),
        Outcome::Stopped { .. }
        | Outcome::Declined
        | Outcome::WouldOverwrite
        | Outcome::Unmanaged => {}
    }
    conditions::save(ctx, &conditions);

    let mut history = attempts::load(ctx);
    history.record(Entry {
        at: ctx.stamp(),
        check: repair.check.clone(),
        did: repair.does.clone(),
        outcome: outcome.clone(),
    });
    attempts::save(ctx, &history);
}
