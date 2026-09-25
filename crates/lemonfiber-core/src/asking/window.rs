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
mod tests;
