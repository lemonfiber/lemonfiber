//! An instant: lemonfiber's own, and as the services write one.
//!
//! Every instant lemonfiber reads or writes is a whole number of seconds since the Unix
//! epoch, an [`Instant`]: a JSON integer in a body, its decimal digits in a link's query.
//! One type, so a field holding one says so in its type and the contract publishes the
//! same integer for every one of them.
//!
//! Every service in the stack keeps its own history stamped in ISO-8601 — the date, the
//! time to the second, in UTC — and a window asked for in any other frame lines up with
//! none of the rows it will be compared against. So the two meet here as well: one
//! function that writes an instant the way a service reads one, and one that reads
//! what a service wrote.
//!
//! Fractions of a second are accepted and dropped. What is wanted is the second something
//! happened, and a service recording six decimal places is not offering more certainty
//! than that about when it last queried an indexer. An explicit zone is a different
//! matter: a stamp that names an offset is not the frame this reads, and guessing at it
//! would put a window hours away from the rows it is meant to cover, so it is not read at
//! all rather than read wrongly.

use std::borrow::Cow;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lemonfiber_manifest::Date;
use serde::{Deserialize, Deserializer, Serialize};

/// Seconds in a day.
pub const DAY: u64 = 86_400;

/// Seconds in an hour.
pub const HOUR: u64 = 3_600;

/// Seconds in a minute.
pub const MINUTE: u64 = 60;

/// The day the epoch counts from, which is how a date becomes a count of days.
const EPOCH: Date = Date {
    year: 1970,
    month: 1,
    day: 1,
};

/// A moment, as a whole number of seconds since the Unix epoch.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Instant(u64);

impl Instant {
    /// The epoch itself.
    pub const EPOCH: Self = Self(0);

    /// The instant `seconds` after the epoch.
    #[must_use]
    pub const fn from_seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    /// The whole seconds since the epoch.
    #[must_use]
    pub const fn seconds(self) -> u64 {
        self.0
    }

    /// The second `at` falls in, or the epoch for a moment before it.
    #[must_use]
    pub fn of(at: SystemTime) -> Self {
        Self(
            at.duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_secs()),
        )
    }

    /// The instant ISO-8601 text in UTC names, as a service writes one and [`read`] reads
    /// it: a fraction of any length and a trailing `Z` are taken, an explicit offset is
    /// refused.
    #[must_use]
    pub fn from_service_text(stamp: &str) -> Option<Self> {
        read(stamp).map(Self::of)
    }

    /// The same moment as the system clock holds one.
    #[must_use]
    pub fn time(self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.0)
    }

    /// The instant `seconds` later, held at the last one there is.
    #[must_use]
    pub const fn later(self, seconds: u64) -> Self {
        Self(self.0.saturating_add(seconds))
    }

    /// The instant `seconds` earlier, held at the epoch.
    #[must_use]
    pub const fn earlier(self, seconds: u64) -> Self {
        Self(self.0.saturating_sub(seconds))
    }

    /// The whole seconds from `before` to this, or none where `before` is later.
    #[must_use]
    pub const fn since(self, before: Self) -> u64 {
        self.0.saturating_sub(before.0)
    }
}

/// Written as its decimal digits, the shape it takes in a link's query.
impl std::fmt::Display for Instant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Read from its decimal digits, the shape [`Display`](std::fmt::Display) writes.
impl std::str::FromStr for Instant {
    type Err = std::num::ParseIntError;

    fn from_str(digits: &str) -> Result<Self, Self::Err> {
        digits.parse().map(Self)
    }
}

impl schemars::JsonSchema for Instant {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "Instant".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        u64::json_schema(generator)
    }
}

/// What a file on disk may hold where an instant is kept.
#[derive(Deserialize)]
#[serde(untagged)]
enum Kept {
    Seconds(u64),
    Text(String),
}

impl Kept {
    /// The instant it holds; the empty text an earlier build wrote for a clock it could
    /// not place reads as the epoch, as [`Instant::of`] reads that clock now.
    fn instant(&self) -> Option<Instant> {
        match self {
            Self::Seconds(seconds) => Some(Instant(*seconds)),
            Self::Text(text) if text.is_empty() => Some(Instant::EPOCH),
            Self::Text(text) if text.bytes().all(|byte| byte.is_ascii_digit()) => {
                text.parse().ok().map(Instant)
            }
            Self::Text(text) => Instant::from_service_text(text),
        }
    }
}

/// An instant kept in a file on disk: an integer, a string of its digits, or the ISO
/// text [`written`] writes, so a file an earlier build wrote still reads.
///
/// # Errors
///
/// Where the value is none of the three.
pub fn kept<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Instant, D::Error> {
    Kept::deserialize(deserializer)?
        .instant()
        .ok_or_else(|| serde::de::Error::custom("not an instant"))
}

/// An instant kept in a file on disk, where one may be absent: [`kept`], or null.
///
/// # Errors
///
/// Where a value is present and none of the three forms [`kept`] reads.
pub fn kept_option<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Instant>, D::Error> {
    Option::<Kept>::deserialize(deserializer)?
        .map(|kept| {
            kept.instant()
                .ok_or_else(|| serde::de::Error::custom("not an instant"))
        })
        .transpose()
}

/// An instant an adapter answered with, where it answered one: whole seconds, or the ISO
/// text the adapters the catalogue pins still answer with, read as [`kept_option`] reads
/// a file.
///
/// # Errors
///
/// Where a value is present and is neither.
pub fn until_adapters_answer_integers<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Instant>, D::Error> {
    kept_option(deserializer)
}

/// Instants kept in a file on disk by name, each read as [`kept`] reads one.
///
/// # Errors
///
/// Where a value is none of the three forms [`kept`] reads.
pub fn kept_each<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<std::collections::BTreeMap<String, Instant>, D::Error> {
    std::collections::BTreeMap::<String, Kept>::deserialize(deserializer)?
        .into_iter()
        .map(|(name, kept)| {
            kept.instant()
                .map(|instant| (name, instant))
                .ok_or_else(|| serde::de::Error::custom("not an instant"))
        })
        .collect()
}

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
