//! The egress guard a stack's torrent traffic leaves through once the stack's own has
//! moved into its plugin.
//!
//! Gluetun's first-party plugin as the bundle pins it, recorded where a context keeps its
//! plugins, and its adapter keyed where its configuration is written and published on the
//! host's loopback, answering `network.egress-guard` over whatever transport the test
//! scripts.

use lemonfiber_contract::capabilities::network::egress;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::plugin::Installed;
use lemonfiber_fixtures::http::Answer;
use lemonfiber_fixtures::support::Reporting;

use super::adapted::{answered, path_of, Adapted, PORT};

/// The plugin the guard moved into.
pub const PLUGIN: &str = "gluetun";

/// The service speaking `network.egress-guard`.
pub const ADAPTER: &str = "gluetun-adapter";

/// The guard's plugin and the adapter in front of it.
const MOVED: Adapted = Adapted {
    plugin: PLUGIN,
    adapter: ADAPTER,
    port: PORT,
};

/// Gluetun's plugin as the bundle pins it, recorded as installed from its embedded tree.
#[must_use]
pub fn moved() -> Option<Installed> {
    MOVED.installed()
}

/// Where `operation` of `network.egress-guard` is asked.
#[must_use]
pub fn operation(operation: &str) -> &'static str {
    path_of(egress::CAPABILITY, egress::MAJOR, operation)
}

/// The route answering the guard's `forwarded` with `port`.
#[must_use]
pub fn forwarding(port: Option<u16>) -> (&'static str, Answer) {
    (operation("forwarded"), answered(&port))
}

/// The engine's account of the guard's adapter running healthy, published on its port.
#[must_use]
pub fn running() -> Reporting {
    MOVED.running()
}

/// `ctx` with the guard moved into its plugin without writing to its stack: the plugin
/// recorded beside its env file, and [`ADAPTER`]'s key held wherever its configuration
/// directory is. The engine stays `ctx`'s own.
#[must_use]
pub fn keyed(ctx: Ctx) -> Ctx {
    MOVED.keyed(ctx)
}
