//! The subtitle finder a stack asks once the stack's own has moved into its plugin.
//!
//! Bazarr's first-party plugin as the bundle pins it, recorded where a context keeps its
//! plugins: the plugin's Bazarr standing in for the stack's, and its adapter keyed where
//! its configuration is written and published on the host's loopback, answering
//! `subtitles.fetch` over whatever transport the test scripts. [`operation`] names the
//! path each operation is asked at, so a script answers the contract rather than the
//! finder behind it.

use lemonfiber_contract::capabilities::subtitles::fetch;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::plugin::Installed;
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::ports::service::{Watched, Watching};
use lemonfiber_fixtures::http::Answer;
use lemonfiber_fixtures::support::Reporting;

pub use super::adapted::{answered, refusing, taken, unavailable};
use super::adapted::{asked_at, path_of, Adapted, PORT};

/// The plugin the subtitle finder moved into.
pub const PLUGIN: &str = "bazarr";

/// The service speaking `subtitles.fetch`.
pub const ADAPTER: &str = "bazarr-adapter";

/// The subtitle finder's plugin and the adapter in front of it.
pub const MOVED: Adapted = Adapted {
    plugin: PLUGIN,
    adapter: ADAPTER,
    port: PORT,
};

/// Where `operation` of `subtitles.fetch` is asked.
#[must_use]
pub fn operation(operation: &str) -> &'static str {
    path_of(fetch::CAPABILITY, fetch::MAJOR, operation)
}

/// Bazarr's plugin as the bundle pins it, recorded as installed from its embedded tree.
#[must_use]
pub fn moved() -> Option<Installed> {
    MOVED.installed()
}

/// The engine's account of [`ADAPTER`] running healthy, published on the adapters' port.
#[must_use]
pub fn running() -> Reporting {
    MOVED.running()
}

/// `ctx` with its subtitle finder moved into its plugin without writing to its stack, for
/// a stack directory tests share: [`moved`] recorded beside its env file, and [`ADAPTER`]'s
/// key held wherever its configuration directory is. The engine stays `ctx`'s own.
#[must_use]
pub fn keyed(ctx: Ctx) -> Ctx {
    MOVED.keyed(ctx)
}

/// `ctx` reaching its subtitle finder moved into its plugin: [`moved`] recorded beside the
/// env file its settings name, with every plugin recorded there before, [`ADAPTER`] keyed
/// where its configuration is written, and the engine saying it runs as [`running`] does.
#[must_use]
pub fn reaching(ctx: Ctx) -> Ctx {
    MOVED.reaching(ctx)
}

/// `ctx` asking its subtitle finder through fillers it was handed, wherever their stack is
/// written: the engine saying [`ADAPTER`] runs as [`running`] does, and its key held
/// wherever its configuration directory is.
#[must_use]
pub fn asking(ctx: Ctx) -> Ctx {
    MOVED.asking(ctx)
}

/// What the finder holds for a curator it has never been told about: watching nothing,
/// at no address, with no key.
#[must_use]
pub fn unwatched() -> Watching {
    Watching {
        enabled: false,
        host: String::new(),
        port: 0,
        keyed: false,
    }
}

/// What the finder holds for a curator it watches at `host` and `port`, holding a key
/// for it.
#[must_use]
pub fn watching_at(host: &str, port: u16) -> Watching {
    Watching {
        enabled: true,
        host: host.to_owned(),
        port,
        keyed: true,
    }
}

/// The route answering `subtitles.fetch`'s `watching` with `held`.
#[must_use]
pub fn watching(held: &Watching) -> (&'static str, Answer) {
    (operation("watching"), answered(held))
}

/// The route answering `subtitles.fetch`'s `watch` with `answer`.
#[must_use]
pub fn watch(answer: Answer) -> (&'static str, Answer) {
    (operation("watch"), answer)
}

/// Each curator `subtitles.fetch`'s `watch` was told about in `sent`, in order.
#[must_use]
pub fn told(sent: &[Request]) -> Vec<Watched> {
    asked_at(sent, operation("watch"))
        .into_iter()
        .filter_map(|asked| serde_json::from_value(asked.get("watched")?.clone()).ok())
        .collect()
}

#[cfg(test)]
#[path = "subtitles/tests.rs"]
mod tests;
