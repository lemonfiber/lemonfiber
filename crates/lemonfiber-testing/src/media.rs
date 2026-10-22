//! The media server a household is reached through once the stack's own has moved into
//! its plugin.
//!
//! Jellyfin's first-party plugin as the bundle pins it, recorded where a context keeps
//! its plugins: the plugin's Jellyfin standing in for the stack's, and its adapter keyed
//! where its configuration is written and published on the host's loopback, answering the
//! media server's contracts over whatever transport the test scripts. [`identity`] and
//! [`serve`] name the path each operation is asked at, so a script answers the contract
//! rather than the server behind it.

use std::sync::Arc;

use lemonfiber_contract::capabilities::{identity::source, media::serve};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::plugin::{Installed, Register};
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::filesystem::{Beneath, Confined, Fault, Lying};
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::ports::service::{Access, Certificate, Invited, Member, NamedLibrary};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Answer;
use lemonfiber_fixtures::support::Reporting;

/// The plugin the media server moved into.
pub const PLUGIN: &str = "jellyfin";

/// The service speaking the media server's contracts.
pub const ADAPTER: &str = "jellyfin-adapter";

/// The port [`ADAPTER`] is published on, on the host's loopback.
pub const PORT: u16 = 8080;

/// The key [`ADAPTER`] is asked with.
pub const KEY: &str = "the-adapter-key";

/// Where `operation` of `identity.source` is asked.
#[must_use]
pub fn identity(operation: &str) -> &'static str {
    asked_at(source::CAPABILITY, source::MAJOR, operation)
}

/// Where `operation` of `media.serve` is asked.
#[must_use]
pub fn serve(operation: &str) -> &'static str {
    asked_at(serve::CAPABILITY, serve::MAJOR, operation)
}

/// Where `operation` of `capability` is asked in `major`, held for as long as a test runs.
fn asked_at(capability: &str, major: u32, operation: &str) -> &'static str {
    Box::leak(lemonfiber_contract::path(capability, major, operation).into_boxed_str())
}

/// Jellyfin's plugin as the bundle pins it, recorded as installed from its embedded tree.
#[must_use]
pub fn moved() -> Option<Installed> {
    let pinned = lemonfiber_bundled::PINNED
        .iter()
        .find(|one| one.first_party.plugin == PLUGIN)?;
    let manifest = lemonfiber_plugin::Manifest::from_toml(pinned.copy).ok()?;
    let mut installed = Installed::of(&manifest);
    pinned
        .first_party
        .manifest
        .clone_into(&mut installed.manifest);
    pinned.first_party.tree.clone_into(&mut installed.tree);
    Some(installed)
}

/// The engine's account of [`ADAPTER`] running healthy, published on [`PORT`].
#[must_use]
pub fn running() -> Reporting {
    Reporting::holding(&[ADAPTER], Lifecycle::Running, Health::Healthy).publishing(&[(
        ADAPTER,
        "127.0.0.1",
        PORT,
    )])
}

/// Where [`ADAPTER`]'s key is written in `ctx`'s stack: nothing where `ctx` names no
/// stack directory.
#[must_use]
pub fn key_at(ctx: &Ctx) -> Option<std::path::PathBuf> {
    let project = match ctx.stack {
        Source::External(path) => Some(path.to_path_buf()),
        Source::Embedded(_) => ctx.settings.stack_dir.clone(),
    }?;
    Some(lemonfiber_core::plugin::key_file(&project, ADAPTER))
}

/// `ctx`'s stack with the media server moved in: [`moved`] recorded beside the env file
/// its settings name, with every plugin recorded there before, and [`ADAPTER`] keyed
/// where its configuration is written. The register is left alone where `ctx` names no
/// env file, and the key where it names no stack directory.
pub fn moved_in(ctx: &Ctx) {
    if let Some(key) = key_at(ctx) {
        let _ = std::fs::create_dir_all(key.parent().unwrap_or(&key));
        let _ = std::fs::write(&key, KEY);
    }
    let Some(kept) = ctx
        .settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name(lemonfiber_core::config::paths::PLUGINS))
    else {
        return;
    };
    let mut register = std::fs::read_to_string(&kept)
        .ok()
        .and_then(|text| Register::parse(&text).ok())
        .unwrap_or_else(Register::empty);
    if let Some(moved) = moved() {
        let _ = register.record(moved);
    }
    let _ = std::fs::write(&kept, register.to_json().unwrap_or_default());
}

/// `ctx` reaching its media server moved into its plugin: [`moved_in`], with the engine
/// saying [`ADAPTER`] runs as [`running`] does.
#[must_use]
pub fn reaching(mut ctx: Ctx) -> Ctx {
    ctx.seams.engine = Arc::new(running());
    moved_in(&ctx);
    ctx
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

/// `value` as the media server answers it over its contracts.
#[must_use]
pub fn answered<T: serde::Serialize>(value: &T) -> Answer {
    Answer::reply(200, serde_json::to_string(value).unwrap_or_default())
}

/// What the media server answers a call it refuses for `kind`, as the contract declares a
/// refusal.
#[must_use]
pub fn refused(kind: lemonfiber_contract::wire::Kind) -> Answer {
    let refusal = lemonfiber_contract::wire::Refusal::new(kind, "refused in a test");
    Answer::served(
        kind.status(),
        lemonfiber_contract::wire::PROBLEM,
        serde_json::to_string(&refusal).unwrap_or_default(),
    )
}

/// What the media server answers a call while its upstream is not answering.
#[must_use]
pub fn unavailable() -> Answer {
    refused(lemonfiber_contract::wire::Kind::Unavailable)
}

/// What the media server answers a call its upstream answered by refusing.
#[must_use]
pub fn refusing() -> Answer {
    refused(lemonfiber_contract::wire::Kind::Refused)
}

/// What the media server answers a write it took.
#[must_use]
pub fn taken() -> Answer {
    Answer::reply(204, "")
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

/// A disk holding a key for each of its services wherever that service's configuration
/// directory is, and reading every other file as `inner` does: for a stack whose directory
/// a test cannot write.
pub struct Keyed {
    inner: Arc<dyn Confined>,
    services: Vec<String>,
}

impl Keyed {
    /// `inner`, with [`ADAPTER`]'s key held wherever its configuration directory is.
    #[must_use]
    pub fn over(inner: Arc<dyn Confined>) -> Arc<Self> {
        Self::holding(inner, &[ADAPTER])
    }

    /// `inner`, with a key held for each of `services` wherever its configuration
    /// directory is.
    #[must_use]
    pub fn holding(inner: Arc<dyn Confined>, services: &[&str]) -> Arc<Self> {
        Arc::new(Self {
            inner,
            services: services.iter().map(|one| (*one).to_owned()).collect(),
        })
    }

    /// Whether `path` is where one of its services' keys is written in some stack.
    fn holds(&self, path: &std::path::Path) -> bool {
        self.services.iter().any(|service| {
            path.ends_with(
                std::path::Path::new(service).join(lemonfiber_contract::adapter::KEY_FILE),
            )
        })
    }
}

impl Confined for Keyed {
    fn read(&self, path: &std::path::Path, within: &std::path::Path) -> Beneath {
        if self.holds(path) {
            return Beneath::Read(KEY.to_owned());
        }
        self.inner.read(path, within)
    }

    fn overwrite(
        &self,
        path: &std::path::Path,
        within: &std::path::Path,
        contents: &[u8],
    ) -> Result<(), Fault> {
        self.inner.overwrite(path, within, contents)
    }

    fn lies(&self, path: &std::path::Path, within: &std::path::Path) -> Lying {
        self.inner.lies(path, within)
    }

    fn moved(
        &self,
        from: &std::path::Path,
        to: &std::path::Path,
        within: &std::path::Path,
    ) -> Result<(), Fault> {
        self.inner.moved(from, to, within)
    }
}

/// `ctx` asking its media server moved into its plugin through `fillers` it was handed,
/// wherever their stack is written: the engine saying [`ADAPTER`] runs as [`running`]
/// does, and [`ADAPTER`]'s key held wherever its configuration directory is.
#[must_use]
pub fn asking(mut ctx: Ctx) -> Ctx {
    ctx.seams.engine = Arc::new(running());
    ctx.seams.confined = Keyed::over(Arc::clone(&ctx.seams.confined));
    ctx
}

/// `ctx` with its media server moved into its plugin wherever its stack is written:
/// [`moved_in`], and [`ADAPTER`]'s key held wherever its configuration directory is. The
/// engine stays `ctx`'s own, which says [`ADAPTER`] runs where it holds [`running`].
#[must_use]
pub fn keyed_in(mut ctx: Ctx) -> Ctx {
    ctx.seams.confined = Keyed::over(Arc::clone(&ctx.seams.confined));
    moved_in(&ctx);
    ctx
}

/// What `operation` of `identity.source` was asked in `sent`, in order.
#[must_use]
pub fn asked(sent: &[Request], operation: &str) -> Vec<serde_json::Value> {
    let at = identity(operation);
    sent.iter()
        .filter(|request| request.url.ends_with(at))
        .filter_map(|request| serde_json::from_str(request.body.as_deref()?).ok())
        .collect()
}

/// The accounts `operation` of `identity.source` was asked about in `sent`, in order.
#[must_use]
pub fn asked_about(sent: &[Request], operation: &str) -> Vec<String> {
    asked(sent, operation)
        .iter()
        .filter_map(|asked| asked.get("id")?.as_str().map(str::to_owned))
        .collect()
}
