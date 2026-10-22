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
use lemonfiber_core::plugin::{Installed, Register};
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::stack::Source;
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

/// `ctx`'s stack with the media server moved in: [`moved`] recorded beside the env file
/// its settings name, with every plugin recorded there before, and [`ADAPTER`] keyed
/// where its configuration is written. The register is left alone where `ctx` names no
/// env file, and the key where it names no stack directory.
pub fn moved_in(ctx: &Ctx) {
    let project = match ctx.stack {
        Source::External(path) => Some(path.to_path_buf()),
        Source::Embedded(_) => ctx.settings.stack_dir.clone(),
    };
    if let Some(project) = project {
        let key = lemonfiber_core::plugin::key_file(&project, ADAPTER);
        let _ = std::fs::create_dir_all(key.parent().unwrap_or(&project));
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
