//! How often an adapter that polls its upstream looks for what happened.
//!
//! A value outside its range, or not a whole number, is refused when it is set. One that
//! reached the file some other way is read as the default rather than trusted.

use super::env;
use super::reading::unquoted;
use crate::ports::service::{EVERY, EVERY_DEFAULT};

/// The setting naming how many seconds apart an adapter that polls looks.
pub const EVENTS_POLL_KEY: &str = "LEMONFIBER_EVENTS_POLL";

/// How many seconds apart an adapter that polls looks, as the operator recorded it, or
/// [`EVERY_DEFAULT`] where nothing in [`EVERY`] is recorded.
#[must_use]
pub fn events_poll_from_env(file: &env::EnvFile) -> u32 {
    seconds(file.get(EVENTS_POLL_KEY)).unwrap_or(EVERY_DEFAULT)
}

/// The seconds a recorded value names, where it is a whole number in [`EVERY`].
fn seconds(value: Option<&str>) -> Option<u32> {
    unquoted(value?)
        .parse()
        .ok()
        .filter(|seconds| EVERY.contains(seconds))
}

/// Why `value` will not do for `key`, where `key` is [`EVENTS_POLL_KEY`] and it will not.
pub(crate) fn refusal(key: &str, value: &str) -> Option<String> {
    (key == EVENTS_POLL_KEY && seconds(Some(value)).is_none()).then(|| {
        format!(
            "{key} has to be a whole number from {} to {} seconds",
            EVERY.start(),
            EVERY.end()
        )
    })
}

#[cfg(test)]
mod tests;
