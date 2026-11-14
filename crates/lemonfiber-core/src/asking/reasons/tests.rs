use super::Reasons;
use crate::collected::Reference;
use crate::instant::Instant;
use std::collections::BTreeSet;

/// A moment the calendar holds, for these records to be stamped with.
const AT: Instant = Instant::from_seconds(1_787_000_649);

/// A reason written down comes back under the request it was written for.
#[test]
fn a_reason_comes_back_under_the_request_it_was_written_for() {
    let mut held = Reasons::default();
    assert!(held.is_empty());

    held.keep(
        Reference::Filed(41),
        "we already have it in another form",
        Some(AT),
    );

    assert!(!held.is_empty());
    assert_eq!(
        held.of(Reference::Filed(41))
            .map(|kept| (kept.reason.as_str(), kept.at)),
        Some(("we already have it in another form", Some(AT)))
    );
    assert_eq!(
        held.of(Reference::Filed(42)),
        None,
        "a reason reached a request it is not for"
    );
}

/// The record holds the reason as the check that let it past read it.
///
/// A blank reason is refused before this is reached, and one padded either side is
/// trimmed there — so a record keeping the untrimmed spelling would disagree with
/// the line the operator was shown it in.
#[test]
fn the_reason_is_kept_as_the_operator_was_shown_it() {
    let mut held = Reasons::default();
    held.keep(Reference::Filed(7), "  the disk is nearly full  ", Some(AT));

    assert_eq!(
        held.of(Reference::Filed(7))
            .map(|kept| kept.reason.as_str()),
        Some("the disk is nearly full")
    );
}

/// Ruling on one request twice keeps the answer it was last given.
#[test]
fn a_second_answer_replaces_the_first() {
    let mut held = Reasons::default();
    held.keep(Reference::Filed(7), "not this month", None);
    held.keep(
        Reference::Filed(7),
        "on second thoughts, the disk",
        Some(AT),
    );

    assert_eq!(
        held.of(Reference::Filed(7))
            .map(|kept| kept.reason.as_str()),
        Some("on second thoughts, the disk")
    );
}

/// A reason whose request the service no longer holds is forgotten.
///
/// This is a note beside somebody else's list. A note about a line that is no longer
/// there is only a way to grow a file forever.
#[test]
fn a_reason_whose_request_has_gone_is_forgotten() {
    let mut held = Reasons::default();
    held.keep(Reference::Filed(1), "too new", Some(AT));
    held.keep(Reference::Filed(2), "we have it", Some(AT));
    held.keep(Reference::Filed(3), "no room", None);
    let kept = Reference::Kept(crate::collected::Medium::Music, 1);
    held.keep(kept, "we have it on vinyl", None);

    held.only(&BTreeSet::from([2, 3, 9]));

    assert!(held.of(kept).is_some());
    assert_eq!(held.of(Reference::Filed(1)), None);
    assert!(held.of(Reference::Filed(2)).is_some());
    assert!(held.of(Reference::Filed(3)).is_some());
}

/// It survives being written down and read back.
#[test]
fn it_survives_being_written_down_and_read_back() {
    let mut held = Reasons::default();
    held.keep(
        Reference::Filed(11),
        "the season is only half out",
        Some(AT),
    );

    let written = serde_json::to_string(&held).unwrap_or_default();
    let read: Reasons = serde_json::from_str(&written).unwrap_or_default();

    assert_eq!(read, held, "{written}");
}

/// A record this cannot read is no reasons rather than a failure.
#[test]
fn a_record_that_will_not_read_is_no_reasons() {
    let read: Reasons = serde_json::from_str("not a record").unwrap_or_default();

    assert!(read.is_empty());
}

/// The words are owed until they have been carried, and then never again.
///
/// This is the whole of what stops a household hearing the same thing twice: the
/// attempt is written down whether it reached anybody or not, so a member with
/// nowhere to send to is asked about once.
#[test]
fn words_are_owed_once_and_then_never_again() {
    let mut held = Reasons::default();
    held.keep(Reference::Filed(7), "no room this month", Some(AT));
    assert!(
        held.owed(Reference::Filed(7)),
        "a fresh refusal owes nobody anything"
    );

    held.passed_on(Reference::Filed(7), vec!["Pushover".to_owned()], Some(AT));

    assert!(
        !held.owed(Reference::Filed(7)),
        "the same words are owed a second time"
    );
    assert_eq!(
        held.of(Reference::Filed(7))
            .and_then(|kept| kept.told.clone()),
        Some(super::Passed {
            to: vec!["Pushover".to_owned()],
            at: Some(AT),
        })
    );
}

/// Nowhere to send is still an attempt made, and still not owed again.
#[test]
fn nowhere_to_send_is_still_an_attempt_that_happened() {
    let mut held = Reasons::default();
    held.keep(Reference::Filed(7), "not this month", None);

    held.passed_on(Reference::Filed(7), Vec::new(), None);

    assert!(!held.owed(Reference::Filed(7)));
    assert_eq!(
        held.of(Reference::Filed(7))
            .and_then(|kept| kept.told.clone())
            .map(|passed| passed.to),
        Some(Vec::new())
    );
}

/// A request nothing was kept for is owed nothing and records nothing.
#[test]
fn a_request_with_no_reason_kept_is_owed_nothing() {
    let mut held = Reasons::default();
    assert!(
        !held.owed(Reference::Filed(7)),
        "a request nobody refused here owes words"
    );

    held.passed_on(Reference::Filed(7), vec!["Pushover".to_owned()], None);

    assert_eq!(
        held.of(Reference::Filed(7)),
        None,
        "a delivery invented a refusal"
    );
}

/// A request nobody ruled on is kept apart from one somebody refused.
///
/// The same shape and opposite events. A reading that could not tell them apart
/// would report a household as having been refused things nobody refused, and would
/// tell the member they were turned down when what happened is that they ran out.
#[test]
fn a_request_that_ran_out_is_kept_apart_from_one_somebody_refused() {
    let mut held = Reasons::default();
    held.keep(Reference::Filed(7), "we already have it dubbed", Some(AT));
    held.closed(
        Reference::Filed(9),
        "nobody ruled on it within 30 days",
        Some(AT),
    );

    assert_eq!(
        held.of(Reference::Filed(7)).map(|kept| kept.expired),
        Some(false)
    );
    assert_eq!(
        held.of(Reference::Filed(9)).map(|kept| kept.expired),
        Some(true)
    );
    assert!(
        held.owed(Reference::Filed(9)),
        "a request that ran out owes its words like any other"
    );
}

/// Closing the same request twice owes its words once.
///
/// **The whole of what stops a clock telling a household the same thing every hour**,
/// and it is here rather than in the clock: a second closure is the same sentence
/// about the same silence, so the record it would rewrite is left alone and the words
/// stay carried. An operator's own second answer is the opposite case, below.
#[test]
fn closing_the_same_request_twice_owes_its_words_once() {
    let mut held = Reasons::default();
    held.closed(
        Reference::Filed(7),
        "nobody ruled on it within 30 days",
        Some(AT),
    );
    held.passed_on(Reference::Filed(7), vec!["Pushover".to_owned()], Some(AT));

    held.closed(
        Reference::Filed(7),
        "nobody ruled on it within 30 days",
        Some(AT),
    );

    assert!(
        !held.owed(Reference::Filed(7)),
        "the same words were owed a second time"
    );
    assert_eq!(
        held.of(Reference::Filed(7))
            .and_then(|kept| kept.told.clone()),
        Some(super::Passed {
            to: vec!["Pushover".to_owned()],
            at: Some(AT),
        })
    );
}

/// An operator turning down what ran out is a decision, and owes its own words.
///
/// The other side of the line above: what is refused a second telling is the same
/// silence said again, not somebody actually answering.
#[test]
fn somebody_answering_after_it_ran_out_owes_their_own_words() {
    let mut held = Reasons::default();
    held.closed(
        Reference::Filed(7),
        "nobody ruled on it within 30 days",
        None,
    );
    held.passed_on(Reference::Filed(7), vec!["Pushover".to_owned()], None);

    held.keep(Reference::Filed(7), "and we already have it dubbed", None);

    assert!(held.owed(Reference::Filed(7)));
    assert_eq!(
        held.of(Reference::Filed(7)).map(|kept| kept.expired),
        Some(false)
    );
}

/// A second answer to the same request owes its own words afresh.
///
/// Nothing can reach this today — a request already decided is refused by name
/// before a second answer is taken — and if that ever changed, the new words would
/// be new words rather than ones already carried.
#[test]
fn a_second_answer_owes_its_own_words() {
    let mut held = Reasons::default();
    held.keep(Reference::Filed(7), "not this month", None);
    held.passed_on(Reference::Filed(7), vec!["Pushover".to_owned()], None);

    held.keep(Reference::Filed(7), "on second thoughts, the disk", None);

    assert!(held.owed(Reference::Filed(7)));
}
