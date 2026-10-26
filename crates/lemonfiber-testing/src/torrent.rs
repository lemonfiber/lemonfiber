//! The torrent client a stack downloads through once the stack's own has moved into its
//! plugin.
//!
//! qBittorrent's first-party plugin as the bundle pins it, recorded where a context keeps
//! its plugins: the plugin's qBittorrent standing in for the stack's, and its adapter keyed
//! where its configuration is written and published on the host's loopback, answering
//! `download.torrent` over whatever transport the test scripts. [`operation`] names the
//! path each operation is asked at, so a script answers the contract rather than the
//! client behind it.

use std::time::Duration;

use lemonfiber_contract::capabilities::download::torrent;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::plugin::Installed;
use lemonfiber_core::ports::service::Download;
use lemonfiber_fixtures::http::Answer;
use lemonfiber_fixtures::support::Reporting;

pub use super::adapted::{answered, refusing, taken, unavailable};
use super::adapted::{path_of, Adapted, PORT};

/// The plugin the torrent client moved into.
pub const PLUGIN: &str = "qbittorrent";

/// The service speaking `download.torrent`.
pub const ADAPTER: &str = "qbittorrent-adapter";

/// The torrent client's plugin and the adapter in front of it.
const MOVED: Adapted = Adapted {
    plugin: PLUGIN,
    adapter: ADAPTER,
    port: PORT,
};

/// What the torrent client is carrying in these fixtures.
pub const CARRIED: &str = "Ubuntu.iso";

/// What the torrent client holds finished in these fixtures.
pub const FINISHED: &str = "Done.iso";

/// Where `operation` of `download.torrent` is asked.
#[must_use]
pub fn operation(operation: &str) -> &'static str {
    path_of(torrent::CAPABILITY, torrent::MAJOR, operation)
}

/// qBittorrent's plugin as the bundle pins it, recorded as installed from its embedded
/// tree.
#[must_use]
pub fn moved() -> Option<Installed> {
    MOVED.installed()
}

/// The engine's account of [`ADAPTER`] running healthy, published on the adapters' port.
#[must_use]
pub fn running() -> Reporting {
    MOVED.running()
}

/// `ctx`'s stack with the torrent client moved in: [`moved`] recorded beside the env file
/// its settings name, with every plugin recorded there before, and [`ADAPTER`] keyed where
/// its configuration is written.
pub fn moved_in(ctx: &Ctx) {
    MOVED.moved_in(ctx);
}

/// [`moved`] recorded beside the env file `ctx`'s settings name, with [`ADAPTER`] keyed
/// nowhere yet: a client still on its first start.
pub fn unkeyed(ctx: &Ctx) {
    MOVED.recorded(ctx);
}

/// `ctx` with its torrent client moved into its plugin without writing to its stack, for a
/// stack directory tests share: [`moved`] recorded beside its env file, and [`ADAPTER`]'s
/// key held wherever its configuration directory is. The engine stays `ctx`'s own.
#[must_use]
pub fn keyed(ctx: Ctx) -> Ctx {
    MOVED.keyed(ctx)
}

/// `ctx` reaching its torrent client moved into its plugin: [`moved_in`], with the engine
/// saying [`ADAPTER`] runs as [`running`] does.
#[must_use]
pub fn reaching(ctx: Ctx) -> Ctx {
    MOVED.reaching(ctx)
}

/// One download the torrent client is carrying: [`CARRIED`], thirty per cent done at a
/// known speed, two minutes from finishing, with seven hundred bytes left to write.
#[must_use]
pub fn carrying() -> Download {
    Download {
        name: CARRIED.to_owned(),
        progress: 30,
        speed: Some(4096),
        eta: Some(Duration::from_secs(120)),
        remaining: Some(700),
    }
}

/// One download the torrent client holds finished: [`FINISHED`], all of it written.
///
/// A client keeps completed items in the same list it reports active ones from, so
/// anything asking what stopping would interrupt has to tell them apart.
#[must_use]
pub fn finished() -> Download {
    Download {
        name: FINISHED.to_owned(),
        progress: 100,
        speed: Some(0),
        eta: None,
        remaining: Some(0),
    }
}

/// The route answering `download.torrent`'s `transfers` with `downloads`.
#[must_use]
pub fn transfers(downloads: &[Download]) -> (&'static str, Answer) {
    (operation("transfers"), answered(&downloads))
}

#[cfg(test)]
#[path = "torrent/tests.rs"]
mod tests;
