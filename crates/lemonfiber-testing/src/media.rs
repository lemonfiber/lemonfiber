//! The media server a household is reached through once the stack's own has moved into
//! its plugin.
//!
//! Jellyfin's first-party plugin as the bundle pins it, recorded where a context keeps
//! its plugins: the plugin's Jellyfin standing in for the stack's, and its adapter keyed
//! where its configuration is written and published on the host's loopback, answering the
//! media server's contracts over whatever transport the test scripts. [`identity`] and
//! [`serve`] name the path each operation is asked at, so a script answers the contract
//! rather than the server behind it.

use lemonfiber_contract::capabilities::{identity::source, media::serve};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::plugin::Installed;
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::ports::service::{Access, Certificate, Invited, Member, NamedLibrary};
use lemonfiber_fixtures::http::Answer;
use lemonfiber_fixtures::support::Reporting;

pub use super::adapted::{answered, refused, refusing, taken, unavailable, Keyed, KEY, PORT};
use super::adapted::{asked_at, path_of, Adapted};

/// The plugin the media server moved into.
pub const PLUGIN: &str = "jellyfin";

/// The service speaking the media server's contracts.
pub const ADAPTER: &str = "jellyfin-adapter";

/// The media server's plugin and the adapter in front of it.
const MOVED: Adapted = Adapted {
    plugin: PLUGIN,
    adapter: ADAPTER,
};

/// Where `operation` of `identity.source` is asked.
#[must_use]
pub fn identity(operation: &str) -> &'static str {
    path_of(source::CAPABILITY, source::MAJOR, operation)
}

/// Where `operation` of `media.serve` is asked.
#[must_use]
pub fn serve(operation: &str) -> &'static str {
    path_of(serve::CAPABILITY, serve::MAJOR, operation)
}

/// Jellyfin's plugin as the bundle pins it, recorded as installed from its embedded tree.
#[must_use]
pub fn moved() -> Option<Installed> {
    MOVED.installed()
}

/// The engine's account of [`ADAPTER`] running healthy, published on [`PORT`].
#[must_use]
pub fn running() -> Reporting {
    MOVED.running()
}

/// Where [`ADAPTER`]'s key is written in `ctx`'s stack: nothing where `ctx` names no
/// stack directory.
#[must_use]
pub fn key_at(ctx: &Ctx) -> Option<std::path::PathBuf> {
    MOVED.key_at(ctx)
}

/// `ctx`'s stack with the media server moved in: [`moved`] recorded beside the env file
/// its settings name, with every plugin recorded there before, and [`ADAPTER`] keyed
/// where its configuration is written. The register is left alone where `ctx` names no
/// env file, and the key where it names no stack directory.
pub fn moved_in(ctx: &Ctx) {
    MOVED.moved_in(ctx);
}

/// `ctx` reaching its media server moved into its plugin: [`moved_in`], with the engine
/// saying [`ADAPTER`] runs as [`running`] does.
#[must_use]
pub fn reaching(ctx: Ctx) -> Ctx {
    MOVED.reaching(ctx)
}

/// When every claimed account in these fixtures was last seen.
pub const LAST_SEEN: &str = "2026-01-01T00:00:00Z";

/// Somebody the media server holds an account for, opening every library, who has set a
/// password where `claimed`.
#[must_use]
pub fn member(id: &str, name: &str, claimed: bool) -> Member {
    Member {
        id: id.to_owned(),
        name: name.to_owned(),
        claimed,
        access: Access {
            every_library: true,
            ..Access::default()
        },
        last_seen: claimed.then(|| LAST_SEEN.to_owned()),
    }
}

/// Every identity operation an offer, a reset, a removal or a sweep makes, answered by a
/// server holding `members` that makes `invited` when asked to invite somebody, records
/// `since` as when each invitation was made, names no library, certificate or country,
/// and takes every write.
#[must_use]
pub fn household(
    members: &[Member],
    invited: &Member,
    since: &[Invited],
) -> Vec<(&'static str, Vec<Answer>)> {
    vec![
        (identity("household"), vec![answered(&members)]),
        (identity("when_invited"), vec![answered(&since)]),
        (identity("invite"), vec![answered(invited)]),
        (
            identity("libraries"),
            vec![answered(&Vec::<NamedLibrary>::new())],
        ),
        (
            identity("ratings"),
            vec![answered(&Vec::<Certificate>::new())],
        ),
        (identity("country"), vec![answered(&None::<String>)]),
        (identity("claimable"), vec![taken()]),
        (identity("allow"), vec![taken()]),
        (identity("unclaim"), vec![taken()]),
        (identity("withdraw"), vec![taken()]),
        (identity("suspend"), vec![taken()]),
    ]
}

/// `ctx` asking its media server moved into its plugin through `fillers` it was handed,
/// wherever their stack is written: the engine saying [`ADAPTER`] runs as [`running`]
/// does, and [`ADAPTER`]'s key held wherever its configuration directory is.
#[must_use]
pub fn asking(ctx: Ctx) -> Ctx {
    MOVED.asking(ctx)
}

/// `ctx` with its media server moved into its plugin wherever its stack is written:
/// [`moved_in`], and [`ADAPTER`]'s key held wherever its configuration directory is. The
/// engine stays `ctx`'s own, which says [`ADAPTER`] runs where it holds [`running`].
#[must_use]
pub fn keyed_in(ctx: Ctx) -> Ctx {
    MOVED.keyed_in(ctx)
}

/// What `operation` of `identity.source` was asked in `sent`, in order.
#[must_use]
pub fn asked(sent: &[Request], operation: &str) -> Vec<serde_json::Value> {
    asked_at(sent, identity(operation))
}

/// What `operation` of `media.serve` was asked in `sent`, in order.
#[must_use]
pub fn served(sent: &[Request], operation: &str) -> Vec<serde_json::Value> {
    asked_at(sent, serve(operation))
}

/// The accounts `operation` of `identity.source` was asked about in `sent`, in order.
#[must_use]
pub fn asked_about(sent: &[Request], operation: &str) -> Vec<String> {
    asked(sent, operation)
        .iter()
        .filter_map(|asked| asked.get("id")?.as_str().map(str::to_owned))
        .collect()
}

#[cfg(test)]
#[path = "media/tests.rs"]
mod tests;
