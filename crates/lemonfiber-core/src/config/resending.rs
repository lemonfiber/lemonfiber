//! How long, and how many, actions sent under a key the web API remembers.
//!
//! A client that heard nothing back sends the same action again under the key it
//! sent it with, and the web API answers the second send with the first one's answer.
//! What it remembers to do that is bounded twice per caller, by age and by count, and
//! both bounds are the operator's: a household on a slow link may want longer, and a
//! machine short of memory fewer.
//!
//! A value outside its range, or not a whole number, is refused when it is set. One
//! that reached the file some other way is read as the default rather than trusted,
//! because a window of none or a bound of millions is not a setting anybody meant.

use std::ops::RangeInclusive;
use std::time::Duration;

use super::env;
use super::reading::unquoted;

/// The setting naming how many minutes an attempt is remembered from its first send.
pub const IDEMPOTENCY_MINUTES_KEY: &str = "LEMONFIBER_IDEMPOTENCY_MINUTES";

/// The setting naming how many attempts are remembered for one caller at once.
pub const IDEMPOTENCY_KEYS_KEY: &str = "LEMONFIBER_IDEMPOTENCY_KEYS";

/// The minutes an attempt is remembered where nothing says otherwise.
///
/// Half an hour, the lease the jobs register keeps a name for, so a second send
/// answered with a job's name is answered while that name can still be redeemed.
pub const DEFAULT_MINUTES: u64 = 30;

/// The attempts remembered for one caller where nothing says otherwise.
///
/// Many more than one caller's clients send inside one window, and few enough that
/// what one caller holds stays a few hundred answers.
pub const DEFAULT_KEYS: usize = 256;

/// The minutes the setting may name: at least one, and at most a day.
///
/// A key serves the retry inside one attempt, so a window longer than a day remembers
/// attempts nobody is still making.
pub const MINUTES: RangeInclusive<u64> = 1..=1440;

/// The attempts the setting may name: at least one, and at most 4096.
pub const KEYS: RangeInclusive<usize> = 1..=4096;

/// How long and how many.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resending {
    /// How long an attempt is remembered from its first send.
    pub within: Duration,
    /// How many attempts are remembered for one caller at once.
    pub at_most: usize,
}

impl Default for Resending {
    fn default() -> Self {
        Self {
            within: Duration::from_secs(DEFAULT_MINUTES * 60),
            at_most: DEFAULT_KEYS,
        }
    }
}

impl Resending {
    /// What the operator has recorded, each half its default where it is absent or
    /// not a value its setting may hold.
    #[must_use]
    pub fn from_env(file: &env::EnvFile) -> Self {
        let defaults = Self::default();
        Self {
            within: minutes(file.get(IDEMPOTENCY_MINUTES_KEY))
                .map_or(defaults.within, |minutes| Duration::from_secs(minutes * 60)),
            at_most: keys(file.get(IDEMPOTENCY_KEYS_KEY)).unwrap_or(defaults.at_most),
        }
    }
}

/// The minutes a recorded value names, where it is a whole number in [`MINUTES`].
fn minutes(value: Option<&str>) -> Option<u64> {
    whole(value).filter(|minutes| MINUTES.contains(minutes))
}

/// The attempts a recorded value names, where it is a whole number in [`KEYS`].
fn keys(value: Option<&str>) -> Option<usize> {
    whole(value).filter(|keys| KEYS.contains(keys))
}

/// A recorded value read as a whole number, quotes and whitespace aside.
fn whole<T: std::str::FromStr>(value: Option<&str>) -> Option<T> {
    unquoted(value?).parse().ok()
}

/// Why `value` will not do for `key`, where `key` is one of these two and it will not.
pub(crate) fn refusal(key: &str, value: &str) -> Option<String> {
    let (refused, range) = match key {
        IDEMPOTENCY_MINUTES_KEY => (
            minutes(Some(value)).is_none(),
            format!("{} to {} minutes", MINUTES.start(), MINUTES.end()),
        ),
        IDEMPOTENCY_KEYS_KEY => (
            keys(Some(value)).is_none(),
            format!("{} to {} attempts", KEYS.start(), KEYS.end()),
        ),
        _ => return None,
    };
    refused.then(|| format!("{key} has to be a whole number from {range}"))
}

#[cfg(test)]
mod tests;
