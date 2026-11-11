//! How often an adapter that polls its upstream looks for what happened.
//!
//! A value outside its range, or not a whole number, is refused when it is set. One that
//! reached the file some other way is read as the default rather than trusted.

use std::ops::RangeInclusive;

use super::env;
use super::reading::unquoted;

/// The setting naming how many seconds apart an adapter that polls looks.
pub const EVENTS_POLL_KEY: &str = "LEMONFIBER_EVENTS_POLL";

/// The seconds apart an adapter that polls looks where nothing says otherwise.
pub const DEFAULT_SECONDS: u32 = 60;

/// The seconds the setting may name: no closer than a quarter of a minute, and no
/// further apart than an hour.
pub const SECONDS: RangeInclusive<u32> = 15..=3600;

/// How many seconds apart an adapter that polls looks, as the operator recorded it, or
/// [`DEFAULT_SECONDS`] where nothing in [`SECONDS`] is recorded.
#[must_use]
pub fn events_poll_from_env(file: &env::EnvFile) -> u32 {
    seconds(file.get(EVENTS_POLL_KEY)).unwrap_or(DEFAULT_SECONDS)
}

/// The seconds a recorded value names, where it is a whole number in [`SECONDS`].
fn seconds(value: Option<&str>) -> Option<u32> {
    unquoted(value?)
        .parse()
        .ok()
        .filter(|seconds| SECONDS.contains(seconds))
}

/// Why `value` will not do for `key`, where `key` is [`EVENTS_POLL_KEY`] and it will not.
pub(crate) fn refusal(key: &str, value: &str) -> Option<String> {
    (key == EVENTS_POLL_KEY && seconds(Some(value)).is_none()).then(|| {
        format!(
            "{key} has to be a whole number from {} to {} seconds",
            SECONDS.start(),
            SECONDS.end()
        )
    })
}

#[cfg(test)]
mod tests;
