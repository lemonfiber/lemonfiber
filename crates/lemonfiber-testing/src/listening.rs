//! The listening server a household plays audiobooks from once the stack's own has moved
//! into its plugin.
//!
//! Audiobookshelf's first-party plugin as the bundle pins it, recorded where a context
//! keeps its plugins: the plugin's Audiobookshelf standing in for the stack's, and its
//! adapter keyed where its configuration is written and published on the host's
//! loopback, answering `owner.claim` and `media.serve` over whatever transport the test
//! scripts.

pub use super::adapted::{claiming, held_by};
use super::adapted::{Adapted, PORT};

/// The plugin the listening server moved into.
pub const PLUGIN: &str = "audiobookshelf";

/// The service speaking `owner.claim` and `media.serve`.
pub const ADAPTER: &str = "audiobookshelf-adapter";

/// The listening server's plugin and the adapter in front of it.
pub const MOVED: Adapted = Adapted {
    plugin: PLUGIN,
    adapter: ADAPTER,
    port: PORT,
};

#[cfg(test)]
#[path = "listening/tests.rs"]
mod tests;
