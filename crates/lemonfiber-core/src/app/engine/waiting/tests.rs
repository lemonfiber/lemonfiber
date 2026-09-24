use super::{due, said, EVERY};
use std::time::Duration;

use crate::app::PATIENCE;

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
