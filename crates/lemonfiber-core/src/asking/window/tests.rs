use super::{earliest, frees_up, waiting_for, DAY};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A moment the calendar holds, for the arithmetic to run against.
const ASKED: &str = "2026-08-17T21:04:09";

/// The same moment, as an instant.
fn asked() -> SystemTime {
    crate::instant::read(ASKED).unwrap_or(UNIX_EPOCH)
}

/// Waiting is counted in whole days, and a part of one is not yet a day.
#[test]
fn waiting_is_counted_in_whole_days() {
    assert_eq!(waiting_for(Some(ASKED), asked()), Some(0));
    assert_eq!(
        waiting_for(Some(ASKED), asked() + Duration::from_secs(DAY - 1)),
        Some(0)
    );
    assert_eq!(
        waiting_for(Some(ASKED), asked() + Duration::from_secs(DAY * 9)),
        Some(9)
    );
}

/// A stamp this cannot read, and one in the future, are both no answer.
///
/// A service whose clock is ahead has not been waiting a negative number of days,
/// and a figure derived from that is one nobody could act on.
#[test]
fn a_stamp_this_cannot_place_is_no_answer_rather_than_nought() {
    assert_eq!(waiting_for(None, asked()), None);
    assert_eq!(waiting_for(Some("tuesday"), asked()), None);
    assert_eq!(
        waiting_for(Some(ASKED), asked() - Duration::from_secs(DAY)),
        None
    );
}

/// The window lets go a period after the thing that entered it.
#[test]
fn the_window_lets_go_a_period_after_what_entered_it() {
    assert_eq!(
        frees_up(Some(ASKED), 7).as_deref(),
        Some("2026-08-24T21:04:09")
    );
    assert_eq!(
        frees_up(Some(ASKED), 30).as_deref(),
        Some("2026-09-16T21:04:09")
    );
}

/// Nothing to age out, or nothing readable, is no date rather than today's.
#[test]
fn nothing_to_age_out_is_no_date_rather_than_an_invented_one() {
    assert_eq!(frees_up(None, 7), None);
    assert_eq!(frees_up(Some("soon"), 7), None);
}

/// The earliest is the earliest moment, whatever each stamp is punctuated like.
///
/// Two spellings of one moment sort against each other by their punctuation, and
/// the service writes both.
#[test]
fn the_earliest_is_a_moment_rather_than_a_string() {
    let stamps = [
        "2026-08-19T00:00:00Z",
        "2026-08-17T21:04:09.482913Z",
        "2026-08-18T00:00:00",
    ];

    assert_eq!(earliest(stamps), Some("2026-08-17T21:04:09.482913Z"));
}

/// Nothing readable among them is no earliest, rather than the first one written.
#[test]
fn nothing_readable_among_them_is_no_earliest() {
    assert_eq!(earliest(["soon", "later"]), None);
    assert_eq!(earliest([]), None);
    assert_eq!(earliest(["soon", ASKED]), Some(ASKED));
}
