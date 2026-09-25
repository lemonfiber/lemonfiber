use super::{became_of, held, still_owed, written, Said};
use crate::asking::Reasons;
use crate::ports::service::HouseholdRequest;
use crate::telling::Told;
use crate::test_support::a_context;

/// A moment the calendar holds, for these records to be stamped with.
const AT: &str = "2026-08-17T21:04:09";

/// One request as the service records it, at the two statuses that decide its state.
fn asked(id: i64) -> HouseholdRequest {
    HouseholdRequest {
        id,
        made: Some(AT.to_owned()),
        member: "Ana".to_owned(),
        kind: Some(crate::recyclarr::Kind::Radarr),
        item: None,
        request_status: 1,
        media_status: 2,
    }
}

/// What one delivery came to, built from the two lists it is.
fn told(reached: &[&str], refused: &[&str]) -> Told {
    Told {
        reached: reached.iter().map(|at| (*at).to_owned()).collect(),
        refused: refused.iter().map(|at| (*at).to_owned()).collect(),
    }
}

/// Reaching them says so and stops claiming the words are still owed.
#[test]
fn reaching_them_says_so_and_leaves_nothing_owed() {
    let both = became_of(&told(&["Pushover", "Pushbullet"], &[]));

    assert!(both.contains("Pushover and Pushbullet"), "{both}");
    assert!(!both.contains("yours to pass on"), "{both}");
}

/// Nowhere to send is said as the ordinary thing it is, not as a fault.
#[test]
fn nowhere_to_send_is_said_as_an_absence_rather_than_a_failure() {
    let nowhere = became_of(&told(&[], &[]));

    assert!(nowhere.contains("no address"), "{nowhere}");
    assert!(nowhere.contains("yours to pass on"), "{nowhere}");
}

/// Everything refusing leaves the words with the operator, and names what refused.
#[test]
fn everything_refusing_leaves_the_words_with_the_operator() {
    let refused = became_of(&told(&[], &["Pushover"]));

    assert!(
        refused.starts_with("Pushover would not take it"),
        "{refused}"
    );
    assert!(refused.contains("yours to pass on"), "{refused}");
}

/// One taking it and one refusing is told once, and said as told once.
#[test]
fn one_taking_it_and_one_refusing_is_told_once() {
    let mixed = became_of(&told(&["Pushbullet"], &["Pushover"]));

    assert!(mixed.contains("on Pushbullet"), "{mixed}");
    assert!(mixed.contains("Pushover would not take it"), "{mixed}");
    assert!(mixed.contains("once rather than twice"), "{mixed}");
    assert!(!mixed.contains("yours to pass on"), "{mixed}");
}

/// Words already carried are owed to nobody, and a request nobody refused here
/// owes nothing at all.
#[test]
fn words_carried_once_are_owed_to_nobody() {
    let mut reasons = Reasons::default();
    assert_eq!(still_owed(&reasons, 7), None, "a refusal nobody made");

    reasons.keep(7, "no room this month", None);
    assert_eq!(
        still_owed(&reasons, 7),
        Some("no room this month".to_owned())
    );

    reasons.passed_on(7, vec!["Pushover".to_owned()], None);
    assert_eq!(still_owed(&reasons, 7), None, "the same words twice");
}

/// The record keeps this refusal, trimmed, and only what the service still holds.
#[test]
fn the_record_keeps_this_refusal_and_only_what_the_service_still_holds() {
    let nowhere = a_context().build();

    let kept = held(
        &nowhere,
        7,
        Said::Operators("  no room this month  "),
        &[asked(7)],
    );
    assert_eq!(
        kept.of(7)
            .map(|refusal| (refusal.reason.as_str(), refusal.expired)),
        Some(("no room this month", false))
    );

    let gone = held(
        &nowhere,
        7,
        Said::Operators("no room this month"),
        &[asked(9)],
    );
    assert!(
        gone.is_empty(),
        "a reason whose request the service does not hold was kept"
    );
}

/// A request that ran out is written down as having run out, not as refused.
///
/// The same path and the same record, and the one thing that differs is the one
/// thing the member and the operator each read it for.
#[test]
fn a_request_that_ran_out_is_written_down_as_having_run_out() {
    let nowhere = a_context().build();

    let kept = held(
        &nowhere,
        7,
        Said::RanOut("nobody ruled on it within 30 days"),
        &[asked(7)],
    );

    assert_eq!(
        kept.of(7)
            .map(|refusal| (refusal.reason.as_str(), refusal.expired)),
        Some(("nobody ruled on it within 30 days", true))
    );
}

/// A reason nowhere could hold is said to be in this answer alone.
///
/// The decision itself went through, so this is not a failure — but an operator who
/// closed the window believing the words were kept would find the next reading of
/// the household bare, and the person who asked would never hear why.
#[test]
fn a_reason_that_could_not_be_kept_says_where_it_now_lives() {
    let nowhere = a_context().build();

    let said = written(&nowhere, &Reasons::default());

    assert!(
        said.is_some_and(|said| said.contains("nowhere else")),
        "a reason nothing kept was reported as kept"
    );
}
