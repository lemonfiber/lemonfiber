//! The hours an operator does not want waking for.
//!
//! The one thing this product holds against a time of day. Everything else is instants
//! and calendar days on purpose — a schedule computed here would be a schedule in the
//! wrong hour twice a year — so the window is read in the **zone the stack already
//! names**, the one `TZ` hands every container, and the conversion is left to a
//! timezone database rather than to arithmetic on an offset.
//!
//! What it decides is narrow: whether *now* is inside the window. Whether an alert is
//! loud enough to go anyway is [`crate::alert::Digest::overrides_quiet`]'s answer, and
//! keeping the two apart is what stops a quiet hour from ever swallowing an emergency.

use std::time::SystemTime;

/// A time of day, to the minute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Minute(u16);

impl Minute {
    /// The minute of the day this reads as, or nothing where it does not read as one.
    fn parse(said: &str) -> Option<Self> {
        let (hour, minute) = said.split_once(':')?;
        let hour: u16 = hour.parse().ok()?;
        let minute: u16 = minute.parse().ok()?;
        if hour > 23 || minute > 59 {
            return None;
        }
        Some(Self(hour * 60 + minute))
    }
}

/// The window an operator asked not to be disturbed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quiet {
    /// When it starts.
    from: Minute,
    /// When it ends.
    to: Minute,
    /// The zone the two times of day are read in.
    ///
    /// Carried with the window rather than beside it, so nothing can ask whether an
    /// instant is inside it without saying whose evening it means.
    zone: String,
}

impl Quiet {
    /// The window `said` describes, as `HH:MM-HH:MM`.
    ///
    /// Nothing where it does not read as one. A window that cannot be read is no window
    /// rather than a guess, because guessing here means silence in the wrong hours.
    #[must_use]
    pub fn parse(said: &str, zone: &str) -> Option<Self> {
        let (from, to) = said.trim().split_once('-')?;
        let (from, to) = (Minute::parse(from.trim())?, Minute::parse(to.trim())?);
        if from == to {
            // A window with no width mutes nothing, and one that mutes nothing is
            // better said as no window at all than as a setting that reads as active.
            return None;
        }
        Some(Self {
            from,
            to,
            zone: zone.to_owned(),
        })
    }

    /// Whether `at`, read in `zone`, falls inside the window.
    ///
    /// A zone the database does not know is no window: an operator whose `TZ` is a typo
    /// should be told about their faults at the wrong hour rather than not at all.
    #[must_use]
    pub fn holds(&self, at: SystemTime) -> bool {
        let Some(now) = minute_of_day(at, &self.zone) else {
            return false;
        };
        if self.from < self.to {
            return now >= self.from && now < self.to;
        }
        // Crossing midnight: 22:00–07:00 is late evening *or* early morning, which is
        // one window and not two, and reading it as `from <= now < to` would make it
        // the seventeen hours nobody asked to be quiet for.
        now >= self.from || now < self.to
    }
}

/// What time of day it is in `zone`, where the database knows it.
fn minute_of_day(at: SystemTime, zone: &str) -> Option<Minute> {
    let seconds = at.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs();
    let seconds = i64::try_from(seconds).ok()?;
    let zone = jiff::tz::TimeZone::get(zone).ok()?;
    let zoned = jiff::Timestamp::from_second(seconds).ok()?.to_zoned(zone);
    let hour = u16::try_from(zoned.hour()).ok()?;
    let minute = u16::try_from(zoned.minute()).ok()?;
    Some(Minute(hour * 60 + minute))
}

#[cfg(test)]
mod tests {
    use super::{Minute, Quiet};
    use std::time::{Duration, SystemTime};

    /// An instant, as seconds since the epoch.
    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    /// 2026-01-15, at the given hour UTC — deep winter, so a northern zone is on
    /// standard time and one hour ahead.
    fn winter(hour: u64) -> SystemTime {
        at(1_768_435_200 + hour * 3_600)
    }

    #[test]
    fn a_window_reads_as_two_times_of_day() {
        let read = Quiet::parse("22:00-07:00", "UTC");
        assert_eq!(
            read,
            Some(Quiet {
                from: Minute(22 * 60),
                to: Minute(7 * 60),
                zone: "UTC".to_owned()
            })
        );
    }

    #[test]
    fn a_window_may_be_written_with_spaces_around_it() {
        assert!(Quiet::parse("  09:30 - 17:45  ", "UTC").is_some());
    }

    /// Guessing here means silence in the wrong hours, so anything unreadable is no
    /// window rather than an approximation of one.
    #[test]
    fn anything_that_does_not_read_as_a_window_is_no_window() {
        for said in [
            "",
            "22:00",
            "22-07",
            "25:00-07:00",
            "22:60-07:00",
            "ten-eleven",
        ] {
            assert!(Quiet::parse(said, "UTC").is_none(), "{said}");
        }
    }

    /// A window with no width would read as active while muting nothing.
    #[test]
    fn a_window_with_no_width_is_no_window() {
        assert!(Quiet::parse("22:00-22:00", "UTC").is_none());
    }

    #[test]
    fn a_daytime_window_holds_between_its_ends() {
        let window = Quiet::parse("09:00-17:00", "UTC");
        assert!(
            window.as_ref().is_some_and(|held| held.holds(winter(9))),
            "at nine"
        );
        assert!(
            window.as_ref().is_some_and(|held| held.holds(winter(16))),
            "at four"
        );
        assert!(
            window.as_ref().is_some_and(|held| !held.holds(winter(8))),
            "before it"
        );
        assert!(
            window.as_ref().is_some_and(|held| !held.holds(winter(17))),
            "at its end"
        );
    }

    /// The ordinary case, and the one a naive comparison gets backwards.
    #[test]
    fn a_window_across_midnight_is_one_window_and_not_seventeen_hours() {
        let window = Quiet::parse("22:00-07:00", "UTC");
        assert!(
            window.as_ref().is_some_and(|held| held.holds(winter(23))),
            "late evening"
        );
        assert!(
            window.as_ref().is_some_and(|held| held.holds(winter(3))),
            "small hours"
        );
        assert!(
            window.as_ref().is_some_and(|held| !held.holds(winter(12))),
            "the middle of the day"
        );
        assert!(
            window.as_ref().is_some_and(|held| !held.holds(winter(7))),
            "at its end"
        );
    }

    /// The whole reason the zone is read rather than assumed: the same instant is a
    /// different hour in two places, and the window belongs to the household.
    #[test]
    fn the_same_instant_falls_differently_in_two_zones() {
        let window = Quiet::parse("22:00-07:00", "UTC");
        // 21:30 UTC is 22:30 in Amsterdam in winter — inside there, outside here.
        let evening = winter(21) + std::time::Duration::from_secs(1_800);
        let theirs = Quiet::parse("22:00-07:00", "Europe/Amsterdam");
        assert!(
            theirs.as_ref().is_some_and(|held| held.holds(evening)),
            "in Amsterdam"
        );
        assert!(
            window.as_ref().is_some_and(|held| !held.holds(evening)),
            "in UTC"
        );
    }

    /// A typo in `TZ` must not mute anything: being told at the wrong hour beats not
    /// being told.
    #[test]
    fn a_zone_the_database_does_not_know_holds_nothing() {
        let window = Quiet::parse("00:00-23:59", "Nowhere/Atlantis");
        assert!(window.as_ref().is_some_and(|held| !held.holds(winter(3))));
    }
}
