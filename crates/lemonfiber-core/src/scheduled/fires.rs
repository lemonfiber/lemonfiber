//! Which minutes a schedule fell due in, read on the wall clock of a zone.

use jiff::tz::TimeZone;
use lemonfiber_manifest::schedule::{Cron, WallClock};

use crate::instant::{Instant, DAY, MINUTE};

/// The furthest back a missed time is looked for.
const FURTHEST: u64 = 366 * DAY;

/// The times a schedule fell due in a stretch of minutes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Fired {
    /// How many.
    pub(crate) count: u32,
    /// The last of them.
    pub(crate) last: Option<Instant>,
}

/// The zone the times are read in: the one `named` names, or the host's where it names
/// none the database knows.
pub(crate) fn zone(named: Option<&str>) -> TimeZone {
    named
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .and_then(|name| TimeZone::get(name).ok())
        .unwrap_or_else(TimeZone::system)
}

/// The start of the minute `at` falls in.
pub(crate) const fn minute_of(at: Instant) -> Instant {
    Instant::from_seconds(at.seconds() - at.seconds() % MINUTE)
}

/// The minutes after the one `after` falls in, up to and including the one `until`
/// falls in, that `cron` fires in on `zone`'s wall clock.
pub(crate) fn fired(cron: &Cron, zone: &TimeZone, after: Instant, until: Instant) -> Fired {
    let last = minute_of(until);
    let mut at = Instant::from_seconds(
        minute_of(after)
            .later(MINUTE)
            .seconds()
            .max(last.seconds().saturating_sub(FURTHEST)),
    );
    let mut fired = Fired::default();
    while at <= last {
        if wall_clock(at, zone).is_some_and(|clock| cron.fires(clock)) {
            fired.count = fired.count.saturating_add(1);
            fired.last = Some(at);
        }
        at = at.later(MINUTE);
    }
    fired
}

/// What `at` reads as on `zone`'s wall clock.
fn wall_clock(at: Instant, zone: &TimeZone) -> Option<WallClock> {
    let seconds = i64::try_from(at.seconds()).ok()?;
    let zoned = jiff::Timestamp::from_second(seconds)
        .ok()?
        .to_zoned_datetime(zone.clone());
    Some(WallClock {
        minute: u8::try_from(zoned.minute()).ok()?,
        hour: u8::try_from(zoned.hour()).ok()?,
        day: u8::try_from(zoned.day()).ok()?,
        month: u8::try_from(zoned.month()).ok()?,
        weekday: u8::try_from(zoned.weekday().to_sunday_zero_offset()).ok()?,
    })
}

#[cfg(test)]
mod tests;
