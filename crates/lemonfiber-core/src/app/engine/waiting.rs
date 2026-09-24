//! What a wait says while it is still waiting.
//!
//! The loop next door knows everything worth saying — which services have not
//! settled, how long it has been asking, how long it will go on asking. What is
//! decided here is the two things that make the difference between progress and
//! noise: how often it speaks, and in what words.
//!
//! Neither is about waiting. Both are about reading, and a rule about reading kept
//! beside the poll it interrupts is a rule nobody can check without a running
//! engine.

use std::time::Duration;

/// How often a wait says what it is waiting for.
///
/// Not the poll's own half-second: a line twice a second is scrolled past rather
/// than read, and by the time the wait mattered the operator would have stopped
/// looking. Not a minute either — the question being answered is "has this hung",
/// and a minute of silence is long enough to have already decided that it has.
///
/// Five seconds is about as long as a person will watch a still screen before
/// doubting it, and it makes a full three-minute wait thirty-six lines rather than
/// three hundred and sixty. It divides the poll exactly, so a line lands on a pass
/// that was happening anyway and the wait needs no clock of its own.
const EVERY: Duration = Duration::from_secs(5);

/// The line a wait owes now, or nothing where it has spoken recently enough.
///
/// `spoken` counts the intervals already said and is advanced here, so the caller
/// carries a number rather than a policy — the alternative is a loop that decides
/// when to speak, which is the decision this module exists to hold.
///
/// A wait shorter than [`EVERY`] says nothing at all. A stack that comes up in two
/// seconds has nothing to report, and reporting it anyway would teach the operator
/// that these lines are noise before the day one of them matters.
pub(crate) fn due(
    waiting: &[String],
    waited: Duration,
    patience: Duration,
    spoken: &mut u64,
) -> Option<String> {
    let owed = waited.as_secs() / EVERY.as_secs();
    if owed <= *spoken {
        return None;
    }
    *spoken = owed;
    Some(said(waiting, waited, patience))
}

/// What a wait says at this point in it.
///
/// Every service is named rather than counted. The list is the progress: it is the
/// answer to "what is this waiting for", and it visibly shortens as services settle,
/// which no count of seconds does. It is also what the refusal at the end of the
/// budget will name, so an operator who read the wait recognises the report.
///
/// The elapsed figure is what makes the next line worth reading rather than a
/// reprint of this one, and beside the budget it answers the other half of the
/// question: not only that the wait is still going, but how much of it is left.
fn said(waiting: &[String], waited: Duration, patience: Duration) -> String {
    format!(
        "Still starting: {named} — {so_far} seconds so far, of {budget}.",
        named = waiting.join(", "),
        so_far = waited.as_secs(),
        budget = patience.as_secs(),
    )
}

#[cfg(test)]
mod tests;
