//! The request service a household asks through once the stack's own has moved into its
//! plugin.
//!
//! Seerr's first-party plugin as the bundle pins it, recorded where a context keeps its
//! plugins: the plugin's Seerr standing in for the stack's, and its adapter keyed where
//! its configuration is written and published on a loopback port of its own, answering
//! `request.intake` over whatever transport the test scripts. [`intake`] names the address
//! and the path each operation is asked at, so a script answers the contract rather than
//! the service behind it.

use lemonfiber_contract::capabilities::request::intake;
use lemonfiber_core::ports::http::Request;
use lemonfiber_fixtures::http::Answer;

pub use super::adapted::{answered, refusing, taken, unavailable};
use super::adapted::{asked_at, Adapted};

/// Seerr's plugin and the adapter in front of it.
pub const REQUESTS: Adapted = Adapted {
    plugin: "seerr",
    adapter: "seerr-adapter",
    port: 8105,
};

/// Where `operation` of `request.intake` is asked of the request service.
#[must_use]
pub fn intake(operation: &str) -> &'static str {
    REQUESTS.at(intake::CAPABILITY, intake::MAJOR, operation)
}

/// The route answering the request service's `operation` with `value`.
#[must_use]
pub fn answering<T: serde::Serialize>(operation: &str, value: &T) -> (&'static str, Answer) {
    (intake(operation), answered(value))
}

/// What the request service was asked of `operation` in `sent`, each read from the field
/// `field` of what was posted, in order.
#[must_use]
pub fn told<T: serde::de::DeserializeOwned>(
    sent: &[Request],
    operation: &str,
    field: &str,
) -> Vec<T> {
    asked_at(sent, intake(operation))
        .into_iter()
        .filter_map(|asked| serde_json::from_value(asked.get(field)?.clone()).ok())
        .collect()
}

#[cfg(test)]
#[path = "requests/tests.rs"]
mod tests;
