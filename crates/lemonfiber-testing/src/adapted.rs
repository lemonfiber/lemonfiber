//! A stack service moved into its first-party plugin, asked through the adapter in front
//! of it.
//!
//! The plugin as the bundle pins it, recorded where a context keeps its plugins, and its
//! adapter keyed where its configuration is written and published on the host's loopback,
//! answering its contracts over whatever transport the test scripts. Each moved service a
//! test reaches names its plugin and adapter once, as an [`Adapted`], and asks the rest of
//! it here.

use std::sync::Arc;

use lemonfiber_core::app::Ctx;
use lemonfiber_core::plugin::{Installed, Register};
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::filesystem::{Beneath, Confined, Fault, Lying, Readable};
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Answer;
use lemonfiber_fixtures::support::Reporting;

/// The port every adapter is published on, on the host's loopback: the port each listens
/// on, so the engine's account of it reads as published where it listens.
pub const PORT: u16 = 8080;

/// The key every adapter is asked with.
pub const KEY: &str = "the-adapter-key";

/// A first-party plugin whose adapter stands in front of a service moved out of the
/// stack.
#[derive(Debug, Clone, Copy)]
pub struct Adapted {
    /// The plugin the service moved into.
    pub plugin: &'static str,
    /// The plugin's service speaking the moved service's contracts.
    pub adapter: &'static str,
}

impl Adapted {
    /// The plugin as the bundle pins it, recorded as installed from its embedded tree.
    #[must_use]
    pub fn installed(self) -> Option<Installed> {
        let pinned = lemonfiber_bundled::PINNED
            .iter()
            .find(|one| one.first_party.plugin == self.plugin)?;
        let manifest = lemonfiber_plugin::Manifest::from_toml(pinned.copy).ok()?;
        let mut installed = Installed::of(&manifest);
        pinned
            .first_party
            .manifest
            .clone_into(&mut installed.manifest);
        pinned.first_party.tree.clone_into(&mut installed.tree);
        Some(installed)
    }

    /// The engine's account of the adapter running healthy, published on [`PORT`].
    #[must_use]
    pub fn running(self) -> Reporting {
        Reporting::holding(&[self.adapter], Lifecycle::Running, Health::Healthy).publishing(&[(
            self.adapter,
            "127.0.0.1",
            PORT,
        )])
    }

    /// Where the adapter's key is written in `ctx`'s stack: nothing where `ctx` names no
    /// stack directory.
    #[must_use]
    pub fn key_at(self, ctx: &Ctx) -> Option<std::path::PathBuf> {
        let project = match ctx.stack {
            Source::External(path) => Some(path.to_path_buf()),
            Source::Embedded(_) => ctx.settings.stack_dir.clone(),
        }?;
        Some(lemonfiber_core::plugin::key_file(&project, self.adapter))
    }

    /// `ctx`'s stack with the service moved in: the plugin recorded beside the env file its
    /// settings name, with every plugin recorded there before, and the adapter keyed where
    /// its configuration is written. The register is left alone where `ctx` names no env
    /// file, and the key where it names no stack directory.
    pub fn moved_in(self, ctx: &Ctx) {
        if let Some(key) = self.key_at(ctx) {
            let _ = std::fs::create_dir_all(key.parent().unwrap_or(&key));
            let _ = std::fs::write(&key, KEY);
        }
        self.recorded(ctx);
    }

    /// The plugin recorded beside the env file `ctx`'s settings name, in a directory made
    /// for it where none is yet, with every plugin recorded there before, its adapter keyed
    /// nowhere. Nothing where `ctx` names no env file.
    pub fn recorded(self, ctx: &Ctx) {
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
        if let Some(installed) = self.installed() {
            let _ = register.record(installed);
        }
        let _ = std::fs::create_dir_all(kept.parent().unwrap_or(&kept));
        let _ = std::fs::write(&kept, register.to_json().unwrap_or_default());
    }

    /// `ctx` reaching the moved service: [`Self::moved_in`], with the engine saying the
    /// adapter runs as [`Self::running`] does.
    #[must_use]
    pub fn reaching(self, mut ctx: Ctx) -> Ctx {
        ctx.seams.engine = Arc::new(self.running());
        self.moved_in(&ctx);
        ctx
    }

    /// `ctx` asking the moved service through fillers it was handed, wherever their stack
    /// is written: the engine saying the adapter runs as [`Self::running`] does, and the
    /// adapter's key held wherever its configuration directory is.
    #[must_use]
    pub fn asking(self, mut ctx: Ctx) -> Ctx {
        ctx.seams.engine = Arc::new(self.running());
        ctx.seams.confined = Keyed::holding(Arc::clone(&ctx.seams.confined), &[self.adapter]);
        ctx
    }

    /// `ctx` with the service moved in without writing to its stack, for a stack directory
    /// tests share: [`Self::recorded`], and the adapter's key held wherever its
    /// configuration directory is. The engine stays `ctx`'s own.
    #[must_use]
    pub fn keyed(self, mut ctx: Ctx) -> Ctx {
        ctx.seams.confined = Keyed::holding(Arc::clone(&ctx.seams.confined), &[self.adapter]);
        self.recorded(&ctx);
        ctx
    }

    /// `ctx` with the service moved in wherever its stack is written: [`Self::moved_in`],
    /// and the adapter's key held wherever its configuration directory is. The engine
    /// stays `ctx`'s own, which says the adapter runs where it holds [`Self::running`].
    #[must_use]
    pub fn keyed_in(self, mut ctx: Ctx) -> Ctx {
        ctx.seams.confined = Keyed::holding(Arc::clone(&ctx.seams.confined), &[self.adapter]);
        self.moved_in(&ctx);
        ctx
    }
}

/// Where `operation` of `capability` is asked in `major`, held for as long as a test runs.
#[must_use]
pub fn path_of(capability: &str, major: u32, operation: &str) -> &'static str {
    Box::leak(lemonfiber_contract::path(capability, major, operation).into_boxed_str())
}

/// What was asked at `at` in `sent`, in order.
#[must_use]
pub fn asked_at(sent: &[Request], at: &str) -> Vec<serde_json::Value> {
    sent.iter()
        .filter(|request| request.url.ends_with(at))
        .filter_map(|request| serde_json::from_str(request.body.as_deref()?).ok())
        .collect()
}

/// `value` as an adapter answers it over its contracts.
#[must_use]
pub fn answered<T: serde::Serialize>(value: &T) -> Answer {
    Answer::reply(200, serde_json::to_string(value).unwrap_or_default())
}

/// What an adapter answers a call it refuses for `kind`, as the contract declares a
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

/// What an adapter answers a call while its upstream is not answering.
#[must_use]
pub fn unavailable() -> Answer {
    refused(lemonfiber_contract::wire::Kind::Unavailable)
}

/// What an adapter answers a call its upstream answered by refusing.
#[must_use]
pub fn refusing() -> Answer {
    refused(lemonfiber_contract::wire::Kind::Refused)
}

/// What an adapter answers a write it took.
#[must_use]
pub fn taken() -> Answer {
    Answer::reply(204, "")
}

/// A disk holding [`KEY`] for each of its services wherever that service's configuration
/// directory is, and reading every other file as `inner` does: for a stack whose directory
/// a test cannot write.
pub struct Keyed {
    inner: Arc<dyn Confined>,
    services: Vec<String>,
}

impl Keyed {
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

    fn written(
        &self,
        path: &std::path::Path,
        within: &std::path::Path,
        contents: &[u8],
        readable: Readable,
    ) -> Result<(), Fault> {
        self.inner.written(path, within, contents, readable)
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

#[cfg(test)]
#[path = "adapted/tests.rs"]
mod tests;
