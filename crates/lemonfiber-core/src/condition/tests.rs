use super::{Condition, Fault};
use crate::error::Severity;

/// What the stall check reports, with what it costs and what to do about it.
fn stalled(summary: &str) -> Fault {
    Fault::new(
        "queue.stalled",
        Severity::Warning,
        summary,
        "nothing is arriving for them",
        "check the indexer is answering",
    )
}

/// A condition raised at a fixed moment — the stamps are seconds since the
/// epoch, which is what the clock port hands out.
fn raised() -> Condition {
    Condition::raised(
        "queue.stalled",
        &stalled("two downloads have not moved in an hour"),
        "1000",
    )
}

#[test]
fn a_raised_condition_is_wrong_right_now() {
    let condition = raised();
    assert!(condition.is_raised());
    assert_eq!(condition.recurrences, 0, "a first raise has not recurred");
    assert!(!condition.declined);
}

#[test]
fn how_long_it_has_been_broken_is_measured_from_when_it_broke() {
    // An operator asking how long something has been wrong means since it went
    // wrong, not since it was last looked at — so a re-raise of a standing
    // condition must not restamp it.
    let mut condition = raised();
    condition.raise(&stalled("two downloads have not moved in an hour"), "2000");
    assert_eq!(condition.since, "1000");
    assert_eq!(condition.recurrences, 0, "it never went away");
}

#[test]
fn a_standing_condition_still_takes_a_worsening() {
    // It has not gone away, but it has got worse, and the operator is owed the
    // worse one rather than the words it was first raised with.
    let mut condition = raised();
    let worse = Fault::new(
        "storage.full",
        Severity::Error,
        "the disk is full",
        "nothing can be written until something goes",
        "delete something",
    );
    condition.raise(&worse, "2000");
    assert_eq!(condition.severity, Severity::Error);
    assert_eq!(condition.summary, "the disk is full");
    assert_eq!(
        condition.remedies,
        vec!["delete something".to_owned()],
        "the remedy follows the fault, since the stale one is the wrong one"
    );
    assert_eq!(condition.since, "1000", "still since it broke");
}

#[test]
fn a_condition_that_comes_back_is_a_recurrence_and_not_the_same_one() {
    // A fault that flaps is a different problem from one that has been steadily
    // broken, and only the count tells them apart.
    let mut condition = raised();
    condition.clear("1500");
    assert!(!condition.is_raised());
    assert_eq!(condition.cleared.as_deref(), Some("1500"));

    condition.raise(&stalled("and again"), "2000");
    assert!(condition.is_raised());
    assert_eq!(condition.recurrences, 1);
    assert_eq!(condition.since, "2000", "the new spell");
    assert_eq!(condition.cleared, None);
}

#[test]
fn a_declined_fix_is_offered_again_only_once_the_problem_genuinely_returns() {
    // The difference between offering and nagging.
    let mut condition = raised();
    condition.declined = true;
    condition.raise(&stalled("still stalled"), "1500");
    assert!(
        condition.declined,
        "it never went away, so do not ask again"
    );

    condition.clear("2000");
    condition.raise(&stalled("stalled again"), "2500");
    assert!(!condition.declined, "it came back, so ask again");
}

#[test]
fn clearing_what_is_already_clear_changes_nothing() {
    // A run over a healthy stack must not rewrite the store, or every run
    // produces a change for something that did not happen.
    let mut condition = raised();
    condition.clear("1500");
    let settled = condition.clone();
    condition.clear("2000");
    assert_eq!(condition, settled);
}

#[test]
fn only_something_new_and_bad_enough_is_worth_interrupting_over() {
    // The same fault said every run is a fault an operator stops reading.
    let condition = raised();
    assert!(condition.is_worth_saying(None), "never told");
    assert!(
        !condition.is_worth_saying(Some(0)),
        "already told about this spell"
    );

    let advisory = Condition::raised(
        "x",
        &Fault::new(
            "note",
            Severity::Advisory,
            "a note",
            "nothing is required",
            "read it",
        ),
        "1000",
    );
    assert!(!advisory.is_worth_saying(None), "not worth an interruption");

    let mut cleared = raised();
    cleared.clear("1500");
    assert!(!cleared.is_worth_saying(None), "nothing is wrong");
}

#[test]
fn a_recurrence_is_worth_saying_even_where_the_first_spell_was_told() {
    // It came back; that is news, and the count is what makes it distinguishable
    // from the same standing fault.
    let mut condition = raised();
    condition.clear("1500");
    condition.raise(&stalled("again"), "2000");
    assert!(
        condition.is_worth_saying(Some(0)),
        "told about spell 0, this is 1"
    );
    assert!(!condition.is_worth_saying(Some(1)));
}

#[test]
fn how_long_it_has_been_clear_is_read_from_the_two_stamps() {
    let mut condition = raised();
    assert_eq!(
        condition.settled_for("1090"),
        None,
        "still raised, so it has not settled for any time at all"
    );
    condition.clear("1000");
    assert_eq!(condition.settled_for("1090"), Some(90));
    // A clock that went backwards is no time at all rather than a negative one.
    assert_eq!(condition.settled_for("900"), Some(0));
    // A stamp that cannot be read is unknown, never a confident zero: a caller
    // has to be able to tell "not long" from "cannot say".
    assert_eq!(condition.settled_for("not a stamp"), None);
    condition.cleared = Some("yesterday".to_owned());
    assert_eq!(condition.settled_for("1090"), None);
}

#[test]
fn a_store_written_before_remedies_existed_still_loads() {
    // The new fields default rather than failing the parse, so an existing
    // machine's store is not lost to an upgrade.
    let older = r#"{"check":"queue.stalled","severity":"warning","summary":"stalled",
        "since":"1000","cleared":null,"recurrences":0,"declined":false}"#;
    let parsed = serde_json::from_str::<Condition>(older).ok();
    assert_eq!(
        parsed.map(|condition| (
            condition.remedies,
            condition.caused_by,
            condition.attempts,
            condition.meaning
        )),
        Some((Vec::new(), None, 0, String::new()))
    );
}

#[test]
fn a_condition_from_an_older_store_is_told_what_it_means_the_next_time_it_is_seen() {
    // Rather than a migration nobody would run: raising refreshes the wording
    // whether or not the fault is already standing, so the first refresh after
    // an upgrade fills in what the store was written without.
    let older = r#"{"check":"queue.stalled","severity":"warning","summary":"stalled",
        "since":"1000","cleared":null,"recurrences":0,"declined":false}"#;
    let seen = serde_json::from_str::<Condition>(older).ok().map(|mut it| {
        it.raise(&stalled("two downloads have not moved"), "2000");
        (it.meaning, it.since)
    });
    assert_eq!(
        seen,
        Some(("nothing is arriving for them".to_owned(), "1000".to_owned())),
        "the wording arrives; how long it has been wrong does not move"
    );
}

#[test]
fn a_condition_round_trips_through_its_serialised_form() {
    // It is written between runs, so what comes back has to be what went in.
    let condition = raised();
    let text = serde_json::to_string(&condition).unwrap_or_default();
    assert_eq!(
        serde_json::from_str::<Condition>(&text).ok(),
        Some(condition)
    );
}
