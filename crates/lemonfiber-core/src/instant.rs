//! An instant as the services write one.
//!
//! Every service in the stack keeps its own history stamped in ISO-8601 — the date, the
//! time to the second, in UTC — and a window asked for in any other frame lines up with
//! none of the rows it will be compared against. lemonfiber holds time as an instant and
//! calendars as dates, with no time of day anywhere between them, so the two meet here:
//! one function that writes an instant the way a service reads one, and one that reads
//! what a service wrote.
//!
//! Fractions of a second are accepted and dropped. What is wanted is the second something
//! happened, and a service recording six decimal places is not offering more certainty
//! than that about when it last queried an indexer. An explicit zone is a different
//! matter: a stamp that names an offset is not the frame this reads, and guessing at it
//! would put a window hours away from the rows it is meant to cover, so it is not read at
//! all rather than read wrongly.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lemonfiber_manifest::Date;

/// Seconds in a day.
const DAY: u64 = 86_400;

/// Seconds in an hour.
const HOUR: u64 = 3_600;

/// Seconds in a minute.
const MINUTE: u64 = 60;

/// The day the epoch counts from, which is how a date becomes a count of days.
const EPOCH: Date = Date {
    year: 1970,
    month: 1,
    day: 1,
};

/// `at` as a service writes an instant, or nothing where no calendar holds it.
///
/// A count of seconds too large for the calendar to place is passed on as the largest one
/// there is, so it fails at the one place that can say so rather than at two — a moment
/// out past the end of time is refused either way, and one refusal is enough.
#[must_use]
pub fn written(at: SystemTime) -> Option<String> {
    let seconds = at.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let date = Date::from_unix_seconds(i64::try_from(seconds).unwrap_or(i64::MAX))?;
    let clock = seconds % DAY;
    Some(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        date.year,
        date.month,
        date.day,
        clock / HOUR,
        (clock % HOUR) / MINUTE,
        clock % MINUTE
    ))
}

/// The instant a service wrote, where what it wrote can be read as one.
#[must_use]
pub fn read(stamp: &str) -> Option<SystemTime> {
    let (day, rest) = stamp.split_once('T')?;
    let date = Date::parse(day)?;
    let rest = rest.strip_suffix('Z').unwrap_or(rest);
    let (clock, fraction) = rest.split_once('.').unwrap_or((rest, ""));
    if !fraction.chars().all(|digit| digit.is_ascii_digit()) {
        return None;
    }
    let mut fields = clock.split(':');
    let hours = field(fields.next(), 23)?;
    let minutes = field(fields.next(), 59)?;
    let seconds = field(fields.next(), 59)?;
    if fields.next().is_some() {
        return None;
    }
    let days = u64::try_from(EPOCH.days_until(date)).ok()?;
    Some(UNIX_EPOCH + Duration::from_secs(days * DAY + hours * HOUR + minutes * MINUTE + seconds))
}

/// One field of a clock time, where it is a number within the range it may take.
///
/// Bounded rather than merely parsed, so a stamp that reads as a time but cannot be one
/// is refused here instead of becoming an instant days away from what it says.
fn field(text: Option<&str>, most: u64) -> Option<u64> {
    let value: u64 = text?.parse().ok()?;
    (value <= most).then_some(value)
}

#[cfg(test)]
mod tests;
