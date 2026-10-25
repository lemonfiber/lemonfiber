//! The Usenet client a stack downloads through once the stack's own has moved into its
//! plugin.
//!
//! `SABnzbd`'s first-party plugin as the bundle pins it, recorded where a context keeps its
//! plugins: the plugin's `SABnzbd` standing in for the stack's, and its adapter keyed where
//! its configuration is written and published on the host's loopback, answering
//! `download.usenet` over whatever transport the test scripts. [`operation`] names the
//! path each operation is asked at, so a script answers the contract rather than the
//! client behind it.

use std::sync::Arc;
use std::time::Duration;

use lemonfiber_contract::capabilities::download::usenet;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::plugin::Installed;
use lemonfiber_core::ports::service::Download;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Reporting;

use super::adapted::{answered, path_of, Adapted};

/// The plugin the Usenet client moved into.
pub const PLUGIN: &str = "sabnzbd";

/// The service speaking `download.usenet`.
pub const ADAPTER: &str = "sabnzbd-adapter";

/// The Usenet client's plugin and the adapter in front of it.
const MOVED: Adapted = Adapted {
    plugin: PLUGIN,
    adapter: ADAPTER,
};

/// What the Usenet client is carrying in these fixtures.
pub const CARRIED: &str = "Linux.nzb";

/// Where `operation` of `download.usenet` is asked.
#[must_use]
pub fn operation(operation: &str) -> &'static str {
    path_of(usenet::CAPABILITY, usenet::MAJOR, operation)
}

/// `SABnzbd`'s plugin as the bundle pins it, recorded as installed from its embedded tree.
#[must_use]
pub fn moved() -> Option<Installed> {
    MOVED.installed()
}

/// The engine's account of [`ADAPTER`] running healthy, published on the adapters' port.
#[must_use]
pub fn running() -> Reporting {
    MOVED.running()
}

/// `ctx`'s stack with the Usenet client moved in: [`moved`] recorded beside the env file
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

/// `ctx` with its Usenet client moved into its plugin without writing to its stack, for a
/// stack directory tests share: [`moved`] recorded beside its env file, and [`ADAPTER`]'s
/// key held wherever its configuration directory is. The engine stays `ctx`'s own.
#[must_use]
pub fn keyed(ctx: Ctx) -> Ctx {
    MOVED.keyed(ctx)
}

/// `ctx` reaching its Usenet client moved into its plugin: [`moved_in`], with the engine
/// saying [`ADAPTER`] runs as [`running`] does.
#[must_use]
pub fn reaching(ctx: Ctx) -> Ctx {
    MOVED.reaching(ctx)
}

/// One download the Usenet client is carrying: [`CARRIED`], a fifth done, five minutes
/// from finishing, at a speed the client could not read and with no figure for what is
/// left.
#[must_use]
pub fn carrying() -> Download {
    Download {
        name: CARRIED.to_owned(),
        progress: 20,
        speed: None,
        eta: Some(Duration::from_secs(300)),
        remaining: None,
    }
}

/// The route answering `download.usenet`'s `transfers` with `downloads`.
#[must_use]
pub fn transfers(downloads: &[Download]) -> (&'static str, Answer) {
    (operation("transfers"), answered(&downloads))
}

/// `ctx` reaching both download clients moved into their plugins: [`moved_in`] and the
/// torrent client's, with the engine saying both adapters run.
#[must_use]
pub fn reaching_beside_torrents(mut ctx: Ctx) -> Ctx {
    ctx.seams.engine = Arc::new(running().alongside(super::torrent::running()));
    moved_in(&ctx);
    super::torrent::moved_in(&ctx);
    ctx
}

/// A transport answering the torrent client's adapter with `torrents` as what is coming
/// down over torrent, and [`ADAPTER`] with `downloads` as what is coming down over Usenet.
#[must_use]
pub fn beside_torrents(torrents: &[Download], downloads: &[Download]) -> Arc<Fake> {
    Fake::by_path(vec![
        super::torrent::transfers(torrents),
        transfers(downloads),
    ])
}

#[cfg(test)]
#[path = "usenet/tests.rs"]
mod tests;
