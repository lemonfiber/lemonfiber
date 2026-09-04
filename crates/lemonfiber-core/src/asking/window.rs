//! How long somebody has been waiting, and when a period next makes room.
//!
//! **The period is a window that rolls.** The request service counts what was asked for
//! in the last so-many days; nothing resets on a date, and there is no first of the
//! month. What actually happens is that the earliest request still inside the window
//! ages out of it and one more becomes possible — so "when it resets" is a question
//! about that request rather than about a calendar, and it is answered from the
//! requests themselves.
//!
//! Told in whole days rather than to the hour. A member deciding whether to ask now or
//! on Friday is not helped by nineteen hours, and a figure to the hour is one that goes
//! stale between being rendered and being read.

use std::time::{Duration, SystemTime};

/// How long a request may wait before the operator is reminded it is theirs to answer.
///
/// A week. Short enough that somebody who asked has not given up on the house, long
/// enough that a weekend away is not a reminder.
pub const REMINDING_AFTER: u64 = 7;

/// Seconds in a day.
const DAY: u64 = 86_400;

/// How many whole days something asked for at `made` has been waiting at `now`.
///
/// Nothing where the stamp is not one this reads, or where it is in the future: a
/// service whose clock is ahead has not been waiting a negative number of days, and a
/// figure derived from that would be one the operator could not act on.
#[must_use]
pub fn waiting_for(made: Option<&str>, now: SystemTime) -> Option<u64> {
    let asked = crate::instant::read(made?)?;
    Some(now.duration_since(asked).ok()?.as_secs() / DAY)
}

/// When the window lets go of the earliest thing counted in it.
///
/// The moment one more becomes possible, which is what somebody who has run out wants
/// to know. Nothing where no period bounds the count, or where the stamp is not one
/// this reads — a date invented for either would be a promise about a day nothing
/// happens on.
#[must_use]
pub fn frees_up(earliest: Option<&str>, days: u32) -> Option<String> {
    let counted = crate::instant::read(earliest?)?;
    crate::instant::written(counted.checked_add(Duration::from_secs(u64::from(days) * DAY))?)
}

/// The earliest of the stamps given, where any of them is one this reads.
///
/// Compared as instants rather than as text, because the service's own stamps carry a
/// zone marker on some paths and not on others, and two spellings of one moment sort
/// against each other by their punctuation.
#[must_use]
pub fn earliest<'a>(stamps: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    stamps
        .into_iter()
        .filter_map(|stamp| crate::instant::read(stamp).map(|at| (at, stamp)))
        .min_by_key(|(at, _)| *at)
        .map(|(_, stamp)| stamp)
}

#[cfg(test)]
mod tests {
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
}
