use super::Expiry;

/// A moment the calendar holds, for these records to be stamped with.
const AT: &str = "2026-08-17T21:04:09";

/// A household nobody arranged this for closes nothing.
#[test]
fn nothing_is_arranged_until_somebody_arranges_it() {
    let fresh = Expiry::default();

    assert_eq!(fresh.after(), None);
    assert_eq!(fresh.agreed(), None);
}

/// What was agreed to comes back, with the day it was agreed on.
#[test]
fn what_was_agreed_to_comes_back_with_the_day_it_was_agreed_on() {
    let agreed = Expiry::agreed_to(30, Some(AT.to_owned()));

    assert_eq!(agreed.after(), Some(30));
    assert_eq!(agreed.agreed(), Some(AT));
}

/// A clock that could not be written down costs the day and not the period.
#[test]
fn a_clock_that_could_not_be_written_costs_the_day_and_not_the_period() {
    let agreed = Expiry::agreed_to(30, None);

    assert_eq!(agreed.after(), Some(30));
    assert_eq!(agreed.agreed(), None);
}

/// It survives being written down and read back.
#[test]
fn it_survives_being_written_down_and_read_back() {
    let agreed = Expiry::agreed_to(45, Some(AT.to_owned()));

    let written = serde_json::to_string(&agreed).unwrap_or_default();
    let read: Expiry = serde_json::from_str(&written).unwrap_or_default();

    assert_eq!(read, agreed, "{written}");
}

/// A record this cannot read is no arrangement rather than a failure.
///
/// The safe direction, and the only safe direction there is: a file that will not
/// parse leaving a period in force would hold a household to a number nobody could
/// read back.
#[test]
fn a_record_that_will_not_read_is_no_arrangement() {
    let read: Expiry = serde_json::from_str("not a record").unwrap_or_default();

    assert_eq!(read.after(), None);
}

/// A period shorter than the reminder is one nothing is ever reminded about.
#[test]
fn a_period_shorter_than_the_reminder_is_too_soon() {
    assert!(Expiry::too_soon(0));
    assert!(Expiry::too_soon(6));
    assert!(
        !Expiry::too_soon(7),
        "the reminder's own threshold is allowed"
    );
    assert!(!Expiry::too_soon(30));
}
