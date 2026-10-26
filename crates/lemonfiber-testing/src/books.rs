//! The book curator a stack files books with once the stack's own has moved into its
//! plugin.
//!
//! Bindery's first-party plugin as the bundle pins it, recorded where a context keeps its
//! plugins: the plugin's Bindery standing in for the stack's, and its adapter keyed where
//! its configuration is written and published on the host's loopback, answering
//! `indexer.pull` and `library.curate` over whatever transport the test scripts.
//! [`pulling`] and [`curating`] name the path each operation is asked at, so a script
//! answers the contracts rather than the curator behind them.

use lemonfiber_contract::capabilities::indexer::pull;
use lemonfiber_contract::capabilities::library::curate;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::plugin::Installed;
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::ports::service::{
    AggregatorAt, DownloadClient, KnownAggregator, RegisteredClient, RegisteredFolder, RootFolder,
};
use lemonfiber_fixtures::http::Answer;
use lemonfiber_fixtures::support::Reporting;

pub use super::adapted::{answered, refusing, taken, unavailable};
use super::adapted::{asked_at, path_of, Adapted, PORT};

/// The plugin the book curator moved into.
pub const PLUGIN: &str = "bindery";

/// The service speaking `indexer.pull` and `library.curate`.
pub const ADAPTER: &str = "bindery-adapter";

/// The book curator's plugin and the adapter in front of it.
const MOVED: Adapted = Adapted {
    plugin: PLUGIN,
    adapter: ADAPTER,
    port: PORT,
};

/// Where `operation` of `indexer.pull` is asked.
#[must_use]
pub fn pulling(operation: &str) -> &'static str {
    path_of(pull::CAPABILITY, pull::MAJOR, operation)
}

/// Where `operation` of `library.curate` is asked.
#[must_use]
pub fn curating(operation: &str) -> &'static str {
    path_of(curate::CAPABILITY, curate::MAJOR, operation)
}

/// Bindery's plugin as the bundle pins it, recorded as installed from its embedded tree.
#[must_use]
pub fn moved() -> Option<Installed> {
    MOVED.installed()
}

/// The engine's account of [`ADAPTER`] running healthy, published on the adapters' port.
#[must_use]
pub fn running() -> Reporting {
    MOVED.running()
}

/// `ctx` asking its book curator through fillers it was handed, wherever their stack is
/// written: the engine saying [`ADAPTER`] runs as [`running`] does, and its key held
/// wherever its configuration directory is.
#[must_use]
pub fn asking(ctx: Ctx) -> Ctx {
    MOVED.asking(ctx)
}

/// `ctx` with its book curator moved into its plugin without writing to its stack, for a
/// stack directory tests share: [`moved`] recorded beside its env file, and [`ADAPTER`]'s
/// key held wherever its configuration directory is. The engine stays `ctx`'s own.
#[must_use]
pub fn keyed(ctx: Ctx) -> Ctx {
    MOVED.keyed(ctx)
}

/// `ctx` reaching its book curator moved into its plugin: [`moved`] recorded beside the
/// env file its settings name, with every plugin recorded there before, [`ADAPTER`] keyed
/// where its configuration is written, and the engine saying it runs as [`running`] does.
#[must_use]
pub fn reaching(ctx: Ctx) -> Ctx {
    MOVED.reaching(ctx)
}

/// An aggregator the curator reads from at `url`, holding a key for it where `keyed`
/// says.
#[must_use]
pub fn reading_from(url: &str, keyed: bool) -> KnownAggregator {
    KnownAggregator {
        id: "1".to_owned(),
        url: url.to_owned(),
        keyed,
    }
}

/// The route answering `indexer.pull`'s `aggregators` with `held`.
#[must_use]
pub fn aggregators(held: &[KnownAggregator]) -> (&'static str, Answer) {
    (pulling("aggregators"), answered(&held))
}

/// The route answering `indexer.pull`'s `add_aggregator` with `answer`.
#[must_use]
pub fn add_aggregator(answer: Answer) -> (&'static str, Answer) {
    (pulling("add_aggregator"), answer)
}

/// Each aggregator `indexer.pull`'s `add_aggregator` was told about in `sent`, in order.
#[must_use]
pub fn added(sent: &[Request]) -> Vec<AggregatorAt> {
    asked_at(sent, pulling("add_aggregator"))
        .into_iter()
        .filter_map(|asked| serde_json::from_value(asked.get("aggregator")?.clone()).ok())
        .collect()
}

/// The route answering `library.curate`'s `root_folders` with `held`.
#[must_use]
pub fn root_folders(held: &[RegisteredFolder]) -> (&'static str, Answer) {
    (curating("root_folders"), answered(&held))
}

/// The route answering `library.curate`'s `download_clients` with `held`.
#[must_use]
pub fn download_clients(held: &[RegisteredClient]) -> (&'static str, Answer) {
    (curating("download_clients"), answered(&held))
}

/// Each root folder `library.curate`'s `register_root_folder` was told about in `sent`,
/// in order.
#[must_use]
pub fn folders_registered(sent: &[Request]) -> Vec<RootFolder> {
    asked_at(sent, curating("register_root_folder"))
        .into_iter()
        .filter_map(|asked| serde_json::from_value(asked.get("folder")?.clone()).ok())
        .collect()
}

/// Each download client `library.curate`'s `register_download_client` was told about in
/// `sent`, in order.
#[must_use]
pub fn clients_registered(sent: &[Request]) -> Vec<DownloadClient> {
    asked_at(sent, curating("register_download_client"))
        .into_iter()
        .filter_map(|asked| serde_json::from_value(asked.get("client")?.clone()).ok())
        .collect()
}

#[cfg(test)]
#[path = "books/tests.rs"]
mod tests;
