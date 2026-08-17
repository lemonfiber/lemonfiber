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

/// `at` as a service writes an instant, or nothing where it is before the epoch.
#[must_use]
pub fn written(at: SystemTime) -> Option<String> {
    let seconds = at.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let date = Date::from_unix_seconds(i64::try_from(seconds).ok()?)?;
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
mod tests {
    use super::{read, written, Duration, SystemTime, UNIX_EPOCH};

    /// A moment with every field in it, so a formatter that dropped one would show.
    const STAMPED: &str = "2026-08-17T21:04:09";

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn an_instant_survives_being_written_and_read_back() {
        let moment = read(STAMPED).unwrap_or(UNIX_EPOCH);
        assert_eq!(written(moment).as_deref(), Some(STAMPED));
    }

    #[test]
    fn a_stamp_is_read_as_the_second_it_names() {
        assert_eq!(read("1970-01-01T00:00:00"), Some(at(0)));
        assert_eq!(read("1970-01-02T00:00:01"), Some(at(86_401)));
        assert_eq!(read("2026-08-17T00:00:00"), Some(at(1_786_924_800)));
    }

    /// What a service appends to the second is not more certainty about which second it
    /// was, so it is dropped rather than refused.
    #[test]
    fn fractions_and_a_zone_marker_are_dropped_rather_than_refused() {
        let plain = read("2026-08-17T21:04:09");
        assert_eq!(read("2026-08-17T21:04:09Z"), plain);
        assert_eq!(read("2026-08-17T21:04:09.482913Z"), plain);
    }

    /// A stamp naming its own offset is in a frame this does not read, and reading it as
    /// though it were UTC would put a window hours from the rows it is meant to cover.
    #[test]
    fn a_stamp_in_another_frame_is_not_read_at_all() {
        assert_eq!(read("2026-08-17T21:04:09+02:00"), None);
        assert_eq!(read("2026-08-17T21:04:09.48+02:00"), None);
    }

    #[test]
    fn what_cannot_be_a_time_is_not_one() {
        assert_eq!(read(""), None);
        assert_eq!(read("2026-08-17"), None);
        assert_eq!(read("not-a-day T21:04:09"), None);
        assert_eq!(read("2026-08-17T21:04"), None);
        assert_eq!(read("2026-08-17T21:04:09:11"), None);
        assert_eq!(read("2026-08-17T24:00:00"), None);
        assert_eq!(read("2026-08-17T21:60:00"), None);
        assert_eq!(read("2026-08-17T21:04:60"), None);
        assert_eq!(read("2026-08-17Txx:04:09"), None);
    }

    /// Nothing in the stack records anything before the epoch, and an instant that
    /// claims to is not one this can write.
    #[test]
    fn an_instant_before_the_epoch_is_not_written() {
        assert_eq!(written(UNIX_EPOCH - Duration::from_secs(1)), None);
    }
}
