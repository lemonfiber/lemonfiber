//! Trying again before saying anything.
//!
//! A service that is still starting, a connection reset while a container
//! restarts, a name that resolved a moment late — these fail once and work on the
//! next breath. Reporting them is worse than useless: it teaches an operator that
//! lemonfiber cries wolf, and it buries the failures that are real underneath the
//! ones that were not.
//!
//! So a failure that might be a blip is retried, and only what survives is
//! reported — with how many times it was tried, because "it did not answer" and
//! "it did not answer three times over a second" are different claims and the
//! second is the one worth acting on.
//!
//! Bounded deliberately. A diagnostic that a person is watching must stay
//! responsive, so this is a short flurry rather than exponential backoff to a
//! minute: three attempts inside a second, then the truth. Something that needs
//! longer than that to come up is not a blip, and waiting quietly for it would be
//! its own kind of dishonesty.

use std::time::Duration;

/// How many times to try in total, including the first.
///
/// Three is the smallest number that can tell a blip from a pattern: one is no
/// evidence, two could be coincidence, and by the third the answer is the same
/// answer.
pub const ATTEMPTS: u32 = 3;

/// How long the first retry waits. Each subsequent one waits a multiple of it.
///
/// Linear rather than doubling, and derived from the attempt rather than listed,
/// so the waits and the attempt count cannot drift into a gap where a raised
/// [`ATTEMPTS`] quietly buys no extra tries.
const FIRST_WAIT: Duration = Duration::from_millis(200);

/// Whether something is worth trying again, and how long to wait first.
///
/// `attempt` is how many have been made, so the first failure asks with `1`.
/// `None` once the attempts are spent — the failure is the answer.
#[must_use]
pub fn again(attempt: u32) -> Option<Duration> {
    if attempt == 0 || attempt >= ATTEMPTS {
        return None;
    }
    // Each wait a little longer than the last: a service that needs a moment gets
    // one, without the flurry outgrowing the patience of somebody watching.
    Some(FIRST_WAIT.saturating_mul(attempt))
}

/// Whether a failure reported after this many attempts was persistent, as opposed
/// to something nobody tried twice.
///
/// The distinction an operator acts on: a service that did not answer once may
/// have been busy, and one that did not answer every time it was asked is down.
#[must_use]
pub const fn is_persistent(attempts: u32) -> bool {
    attempts >= ATTEMPTS
}

/// How a failure that survived reads, so every surface says it the same way.
///
/// Nothing where it was only tried once — claiming persistence for something
/// nobody retried would be the same overstatement in the other direction.
#[must_use]
pub fn said(attempts: u32) -> Option<String> {
    is_persistent(attempts).then(|| {
        format!(
            "still failing after {attempts} attempt{}",
            crate::plural::s(attempts as usize)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{again, is_persistent, said, ATTEMPTS};

    #[test]
    fn the_first_failures_are_worth_another_go_and_the_last_is_not() {
        assert!(again(1).is_some(), "a first failure may be a blip");
        assert!(again(2).is_some());
        assert_eq!(again(ATTEMPTS), None, "the attempts are spent");
        assert_eq!(again(ATTEMPTS + 1), None, "and stay spent");
    }

    #[test]
    fn nothing_has_been_tried_at_zero_attempts_so_there_is_nothing_to_retry() {
        // Guards the caller that asks before it has tried: a retry policy that
        // answered "wait, then try again" to something never attempted would turn
        // its first attempt into a delayed one.
        assert_eq!(again(0), None);
    }

    #[test]
    fn the_whole_flurry_fits_inside_somebody_watching_a_check_run() {
        // Bounded on purpose. If this ever grows into exponential backoff, a doctor
        // run stops being responsive and the change should be deliberate.
        let total: u128 = (1..ATTEMPTS)
            .filter_map(again)
            .map(|wait| wait.as_millis())
            .sum();
        assert!(total <= 1_000, "{total}ms of waiting");
    }

    #[test]
    fn each_wait_is_longer_than_the_one_before() {
        // A service that needs a moment gets one; a flat interval would either be
        // too short to help or too long to sit through.
        let waits: Vec<u128> = (1..ATTEMPTS)
            .filter_map(again)
            .map(|wait| wait.as_millis())
            .collect();
        let mut ascending = waits.clone();
        ascending.sort_unstable();
        ascending.dedup();
        assert_eq!(waits, ascending, "each wait is longer than the last");
    }

    #[test]
    fn every_attempt_short_of_the_last_gets_a_wait() {
        // Derived from the attempt rather than listed beside it, so raising
        // `ATTEMPTS` cannot leave a gap that quietly buys no extra tries.
        for attempt in 1..ATTEMPTS {
            assert!(again(attempt).is_some(), "attempt {attempt}");
        }
    }

    #[test]
    fn only_something_tried_to_exhaustion_is_called_persistent() {
        assert!(!is_persistent(1), "nobody tried twice");
        assert!(!is_persistent(ATTEMPTS - 1));
        assert!(is_persistent(ATTEMPTS));
    }

    #[test]
    fn a_persistent_failure_says_how_hard_it_was_tried() {
        assert_eq!(
            said(ATTEMPTS).as_deref(),
            Some("still failing after 3 attempts")
        );
        // And one nobody retried claims nothing, which would be the same
        // overstatement in the other direction.
        assert_eq!(said(1), None);
    }
}
