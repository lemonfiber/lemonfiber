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
pub(super) fn due(
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
mod tests {
    use super::{due, said, EVERY};
    use std::time::Duration;

    /// The budget a real start is given, so what these read is what an operator reads.
    const PATIENCE: Duration = Duration::from_secs(180);

    /// Every line one wait produces, asked for at the poll's own rate.
    ///
    /// Driven at half-second steps rather than at the interval, because that is how
    /// the loop asks: a rule that only worked when it was asked on the beat would be
    /// a rule the caller was keeping.
    fn over(seconds: u64, waiting: &[&str]) -> Vec<String> {
        let named: Vec<String> = waiting.iter().map(|id| (*id).to_owned()).collect();
        let mut spoken = 0;
        let mut said = Vec::new();
        for half in 0..=(seconds * 2) {
            let waited = Duration::from_millis(half * 500);
            if let Some(line) = due(&named, waited, PATIENCE, &mut spoken) {
                said.push(line);
            }
        }
        said
    }

    /// The requirement itself: a wait long enough to read as a hang says something,
    /// and says it more than once.
    #[test]
    fn a_wait_speaks_once_every_interval_and_not_between() {
        let said = over(32, &["jellyfin"]);

        assert_eq!(
            said.len(),
            6,
            "six intervals in thirty-two seconds: {said:?}"
        );
    }

    /// A start that settles quickly is not worth remarking on, and remarking on it
    /// is what teaches an operator to ignore the line that matters.
    #[test]
    fn a_wait_shorter_than_the_interval_says_nothing() {
        assert!(over(EVERY.as_secs() - 1, &["jellyfin"]).is_empty());
    }

    /// The first line arrives at the interval rather than at the end of the budget:
    /// the question it answers is asked in the first few seconds of silence.
    #[test]
    fn the_first_line_arrives_one_interval_in() {
        assert_eq!(
            over(EVERY.as_secs(), &["jellyfin"])
                .first()
                .map(String::as_str),
            Some("Still starting: jellyfin — 5 seconds so far, of 180.")
        );
    }

    /// What it is waiting for, in the words the refusal at the end will use — and
    /// how far into the budget it is, which is what makes each line worth reading
    /// rather than a reprint of the one above it.
    #[test]
    fn a_line_names_every_service_it_is_waiting_for() {
        assert_eq!(
            said(
                &["jellyfin".to_owned(), "seerr".to_owned()],
                Duration::from_secs(45),
                PATIENCE
            ),
            "Still starting: jellyfin, seerr — 45 seconds so far, of 180."
        );
    }

    /// Two lines from the same wait differ, because a list that has not changed is
    /// still news when the elapsed figure beside it has.
    #[test]
    fn each_line_says_something_the_one_before_it_did_not() {
        let said = over(10, &["jellyfin"]);
        let first = said.first().map(String::as_str);

        assert_eq!(said.len(), 2, "{said:?}");
        assert!(
            said.last().map(String::as_str) != first,
            "the same list, and a different figure: {said:?}"
        );
    }
}
