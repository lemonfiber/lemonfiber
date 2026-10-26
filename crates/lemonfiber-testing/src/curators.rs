//! The curators a stack files television, film and music with once the stack's own have
//! moved into their plugins.
//!
//! Sonarr's, Radarr's and Lidarr's first-party plugins as the bundle pins them, each
//! recorded where a context keeps its plugins: the plugin's curator standing in for the
//! stack's, and its adapter keyed where its configuration is written and published on a
//! loopback port of its own, answering `library.curate` over whatever transport the test
//! scripts. The three speak one contract on the same paths, so [`curating`] names the
//! address as well as the path an operation is asked at, and a script answers each
//! curator apart.

use lemonfiber_contract::capabilities::library::curate;
use lemonfiber_core::ports::http::Request;
use lemonfiber_fixtures::http::Answer;

pub use super::adapted::{answered, asking, reaching, refusing, running, taken, unavailable};
use super::adapted::{asked_at, path_of, Adapted};

/// Sonarr's plugin and the adapter in front of it.
pub const TELEVISION: Adapted = Adapted {
    plugin: "sonarr",
    adapter: "sonarr-adapter",
    port: 8101,
};

/// Radarr's plugin and the adapter in front of it.
pub const FILM: Adapted = Adapted {
    plugin: "radarr",
    adapter: "radarr-adapter",
    port: 8102,
};

/// Lidarr's plugin and the adapter in front of it.
pub const MUSIC: Adapted = Adapted {
    plugin: "lidarr",
    adapter: "lidarr-adapter",
    port: 8103,
};

/// Every curator moved into its plugin, in the order the bundle pins them.
pub const ALL: [Adapted; 3] = [MUSIC, FILM, TELEVISION];

/// Where `operation` of `library.curate` is asked, of whichever curator.
#[must_use]
pub fn curated(operation: &str) -> &'static str {
    path_of(curate::CAPABILITY, curate::MAJOR, operation)
}

/// Where `operation` of `library.curate` is asked of `curator`.
#[must_use]
pub fn curating(curator: Adapted, operation: &str) -> &'static str {
    curator.at(curate::CAPABILITY, curate::MAJOR, operation)
}

/// The route answering `curator`'s `operation` with `value`.
#[must_use]
pub fn answering<T: serde::Serialize>(
    curator: Adapted,
    operation: &str,
    value: &T,
) -> (&'static str, Answer) {
    (curating(curator, operation), answered(value))
}

/// What `curator` was asked of `operation` in `sent`, each read from the field `field`
/// of what was posted, in order.
#[must_use]
pub fn told<T: serde::de::DeserializeOwned>(
    sent: &[Request],
    curator: Adapted,
    operation: &str,
    field: &str,
) -> Vec<T> {
    asked_at(sent, curating(curator, operation))
        .into_iter()
        .filter_map(|asked| serde_json::from_value(asked.get(field)?.clone()).ok())
        .collect()
}

#[cfg(test)]
#[path = "curators/tests.rs"]
mod tests;
