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
