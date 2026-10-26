//! The indexer a stack searches through once the stack's own has moved into its plugin.
//!
//! Prowlarr's first-party plugin as the bundle pins it, recorded where a context keeps its
//! plugins: the plugin's Prowlarr standing in for the stack's, and its adapter keyed where
//! its configuration is written and published on a loopback port of its own, answering
//! `indexer.search` over whatever transport the test scripts. [`searching`] names the
//! address and the path each operation is asked at, so a script answers the contract
//! rather than the indexer behind it.

use lemonfiber_contract::capabilities::indexer::search;
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::ports::service::Application;
use lemonfiber_fixtures::http::Answer;

pub use super::adapted::{answered, refusing, taken, unavailable};
use super::adapted::{asked_at, Adapted};

/// Prowlarr's plugin and the adapter in front of it.
pub const INDEXER: Adapted = Adapted {
    plugin: "prowlarr",
    adapter: "prowlarr-adapter",
    port: 8104,
};

/// Where `operation` of `indexer.search` is asked of the indexer.
#[must_use]
pub fn searching(operation: &str) -> &'static str {
    INDEXER.at(search::CAPABILITY, search::MAJOR, operation)
}

/// The route answering the indexer's `operation` with `value`.
#[must_use]
pub fn answering<T: serde::Serialize>(operation: &str, value: &T) -> (&'static str, Answer) {
    (searching(operation), answered(value))
}

/// Each application `indexer.search`'s `register_application` was told about in `sent`,
/// in order.
#[must_use]
pub fn registered(sent: &[Request]) -> Vec<Application> {
    asked_at(sent, searching("register_application"))
        .into_iter()
        .filter_map(|asked| serde_json::from_value(asked.get("application")?.clone()).ok())
        .collect()
}

#[cfg(test)]
#[path = "indexer/tests.rs"]
mod tests;
