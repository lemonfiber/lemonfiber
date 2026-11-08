//! How the queue heals: whether it acts, after how many strikes, past what grace, how
//! often it looks, and what counts as slow.
//!
//! A value outside its range, or not a whole number, is refused when it is set. One that
//! reached the file some other way is read as the default rather than trusted, because a
//! strike count of none removes on a single bad reading, which is what strikes exist to
//! stop.

use std::ops::RangeInclusive;
use std::time::Duration;

use super::env;
use super::reading::{reads_as_on, unquoted};
use super::STUCK_SEARCH_AGAIN_KEY;
use crate::instant::{DAY, MINUTE};

/// The setting naming how many consecutive passes flag an item for one reason before it is
/// acted on.
pub const QUEUE_STRIKES_KEY: &str = "LEMONFIBER_QUEUE_STRIKES";

/// The setting naming how many minutes a newly added download is left alone.
pub const QUEUE_GRACE_MINUTES_KEY: &str = "LEMONFIBER_QUEUE_GRACE_MINUTES";

/// The setting naming how many minutes apart the queue is looked at.
pub const QUEUE_EVERY_MINUTES_KEY: &str = "LEMONFIBER_QUEUE_EVERY_MINUTES";

/// The setting naming how many days a download's estimate may run before it is slow.
pub const QUEUE_SLOW_DAYS_KEY: &str = "LEMONFIBER_QUEUE_SLOW_DAYS";

/// The strikes an item takes where nothing says otherwise.
pub const DEFAULT_STRIKES: u32 = 3;

/// The grace a new download is given where nothing says otherwise, in minutes.
pub const DEFAULT_GRACE_MINUTES: u64 = 30;

/// How often the queue is looked at where nothing says otherwise, in minutes.
pub const DEFAULT_EVERY_MINUTES: u64 = 10;

/// The days an estimate may run where nothing says otherwise.
pub const DEFAULT_SLOW_DAYS: u64 = 7;

/// The strikes the setting may name: at least one, and at most twenty.
pub const STRIKES: RangeInclusive<u32> = 1..=20;

/// The grace the setting may name, in minutes: none, up to a week.
pub const GRACE_MINUTES: RangeInclusive<u64> = 0..=10_080;

/// How often the setting may name, in minutes: each minute, up to once a day.
pub const EVERY_MINUTES: RangeInclusive<u64> = 1..=1_440;

/// The days the setting may name before a download is slow: one to ninety.
pub const SLOW_DAYS: RangeInclusive<u64> = 1..=90;

/// How the queue heals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Healing {
    /// Whether it acts, rather than only classifying.
    pub on: bool,
    /// How many consecutive passes flag an item for one reason before it is acted on.
    pub strikes: u32,
    /// How long a newly added download is left alone.
    pub grace: Duration,
    /// How often the queue is looked at.
    pub every: Duration,
    /// How long an estimate may run before the download is slow.
    pub slow: Duration,
}

impl Default for Healing {
    fn default() -> Self {
        Self {
            on: false,
            strikes: DEFAULT_STRIKES,
            grace: Duration::from_secs(DEFAULT_GRACE_MINUTES * MINUTE),
            every: Duration::from_secs(DEFAULT_EVERY_MINUTES * MINUTE),
            slow: Duration::from_secs(DEFAULT_SLOW_DAYS * DAY),
        }
    }
}

impl Healing {
    /// What the operator has recorded, each part its default where it is absent or not a
    /// value its setting may hold.
    #[must_use]
    pub fn from_env(file: &env::EnvFile) -> Self {
        let defaults = Self::default();
        let seconds = |key: &str, range: &RangeInclusive<u64>, unit: u64| {
            whole(file.get(key))
                .filter(|value| range.contains(value))
                .map(|value| Duration::from_secs(value * unit))
        };
        Self {
            on: file.get(STUCK_SEARCH_AGAIN_KEY).is_some_and(reads_as_on),
            strikes: whole(file.get(QUEUE_STRIKES_KEY))
                .filter(|strikes| STRIKES.contains(strikes))
                .unwrap_or(defaults.strikes),
            grace: seconds(QUEUE_GRACE_MINUTES_KEY, &GRACE_MINUTES, MINUTE)
                .unwrap_or(defaults.grace),
            every: seconds(QUEUE_EVERY_MINUTES_KEY, &EVERY_MINUTES, MINUTE)
                .unwrap_or(defaults.every),
            slow: seconds(QUEUE_SLOW_DAYS_KEY, &SLOW_DAYS, DAY).unwrap_or(defaults.slow),
        }
    }
}

/// A recorded value read as a whole number, quotes and whitespace aside.
fn whole<T: std::str::FromStr>(value: Option<&str>) -> Option<T> {
    unquoted(value?).parse().ok()
}

/// Why `value` will not do for `key`, where `key` is one of these four and it will not.
pub(crate) fn refusal(key: &str, value: &str) -> Option<String> {
    let within = |range: &RangeInclusive<u64>| {
        whole::<u64>(Some(value)).is_some_and(|value| range.contains(&value))
    };
    let (held, range) = match key {
        QUEUE_STRIKES_KEY => (
            whole::<u32>(Some(value)).is_some_and(|value| STRIKES.contains(&value)),
            format!("{} to {} strikes", STRIKES.start(), STRIKES.end()),
        ),
        QUEUE_GRACE_MINUTES_KEY => (
            within(&GRACE_MINUTES),
            format!(
                "{} to {} minutes",
                GRACE_MINUTES.start(),
                GRACE_MINUTES.end()
            ),
        ),
        QUEUE_EVERY_MINUTES_KEY => (
            within(&EVERY_MINUTES),
            format!(
                "{} to {} minutes",
                EVERY_MINUTES.start(),
                EVERY_MINUTES.end()
            ),
        ),
        QUEUE_SLOW_DAYS_KEY => (
            within(&SLOW_DAYS),
            format!("{} to {} days", SLOW_DAYS.start(), SLOW_DAYS.end()),
        ),
        _ => return None,
    };
    (!held).then(|| format!("{key} has to be a whole number from {range}"))
}

#[cfg(test)]
mod tests;
