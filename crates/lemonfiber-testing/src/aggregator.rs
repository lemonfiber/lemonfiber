//! The Usenet indexer aggregator a stack searches through once the stack's own has moved
//! into its plugin.
//!
//! `NZBHydra2`'s first-party plugin as the bundle pins it, recorded where a context keeps
//! its plugins: the plugin's `NZBHydra2` standing in for the stack's, and its adapter keyed
//! where its configuration is written and published on the host's loopback, answering
//! `owner.claim` and `indexer.search` over whatever transport the test scripts.

use super::adapted::Adapted;
pub use super::adapted::{claiming, held_by};

/// The plugin the Usenet indexer aggregator moved into.
pub const PLUGIN: &str = "nzbhydra2";

/// The service speaking `owner.claim` and `indexer.search`.
pub const ADAPTER: &str = "nzbhydra2-adapter";

/// The aggregator's plugin and the adapter in front of it.
pub const MOVED: Adapted = Adapted {
    plugin: PLUGIN,
    adapter: ADAPTER,
};

#[cfg(test)]
#[path = "aggregator/tests.rs"]
mod tests;
