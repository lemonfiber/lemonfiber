use super::{frees_up, waiting_for};
use crate::instant::{Instant, DAY};

/// A moment the calendar holds, for the arithmetic to run against: 2026-08-17T21:04:09.
const ASKED: Instant = Instant::from_seconds(1_787_000_649);

/// Waiting is counted in whole days, and a part of one is not yet a day.
#[test]
fn waiting_is_counted_in_whole_days() {
    assert_eq!(waiting_for(Some(ASKED), ASKED), Some(0));
    assert_eq!(waiting_for(Some(ASKED), ASKED.later(DAY - 1)), Some(0));
    assert_eq!(waiting_for(Some(ASKED), ASKED.later(DAY * 9)), Some(9));
}

/// No stamp, and one in the future, are both no answer.
///
/// A service whose clock is ahead has not been waiting a negative number of days,
/// and a figure derived from that is one nobody could act on.
#[test]
fn a_stamp_this_cannot_place_is_no_answer_rather_than_nought() {
    assert_eq!(waiting_for(None, ASKED), None);
    assert_eq!(waiting_for(Some(ASKED), ASKED.earlier(DAY)), None);
}

/// The window lets go a period after the thing that entered it.
#[test]
fn the_window_lets_go_a_period_after_what_entered_it() {
    assert_eq!(
        frees_up(Some(ASKED), 7),
        Some(Instant::from_seconds(1_787_605_449))
    );
    assert_eq!(
        frees_up(Some(ASKED), 30),
        Some(Instant::from_seconds(1_789_592_649))
    );
}

/// Nothing to age out is no date rather than today's.
#[test]
fn nothing_to_age_out_is_no_date_rather_than_an_invented_one() {
    assert_eq!(frees_up(None, 7), None);
}
