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

use crate::instant::{Instant, DAY};

/// How long a request may wait before the operator is reminded it is theirs to answer.
///
/// A week. Short enough that somebody who asked has not given up on the house, long
/// enough that a weekend away is not a reminder.
pub const REMINDING_AFTER: u64 = 7;

/// How many whole days something asked for at `made` has been waiting at `now`.
///
/// Nothing where it is not known when it was asked, or where that is in the future: a
/// service whose clock is ahead has not been waiting a negative number of days, and a
/// figure derived from that would be one the operator could not act on.
#[must_use]
pub fn waiting_for(made: Option<Instant>, now: Instant) -> Option<u64> {
    let made = made?;
    (made <= now).then(|| now.since(made) / DAY)
}

/// When the window lets go of the earliest thing counted in it.
///
/// The moment one more becomes possible, which is what somebody who has run out wants
/// to know. Nothing where no period bounds the count, or where nothing counted is dated
/// — a date invented for either would be a promise about a day nothing happens on.
#[must_use]
pub fn frees_up(earliest: Option<Instant>, days: u32) -> Option<Instant> {
    Some(earliest?.later(u64::from(days) * DAY))
}

#[cfg(test)]
mod tests;
