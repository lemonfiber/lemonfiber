//! The context a test drives a command through.
//!
//! A context is built from every seam, a stack, settings and a platform, of which a
//! test usually varies one or two. This settles the rest, from one of two starting points: [`a_context`], a world that
//! answers from scripts, and [`a_live_context`], one that runs programs and reads the
//! disk for real. What a test varies, it names; what it leaves, it gets from here.
//!
//! The transport, the randomness and the other seams a context holds are not here,
//! because `Ctx` already takes those by name — `with_http`, `with_random` — and this
//! builds a real one to chain from.

use std::path::Path;
use std::sync::Arc;

use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Engine, Images};
use lemonfiber_core::ports::process::Runner;
use lemonfiber_core::ports::{Clock, FileSystem};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::ports::Stopped;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};

/// The stack this repository carries, read from disk.
#[must_use]
pub fn repository_stack() -> Source {
    Source::External(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}

/// A stack source pointing nowhere, for the tests about what happens when it does.
#[must_use]
pub fn nowhere() -> Source {
    Source::External(Path::new("/lemonfiber/no/such/stack"))
}

/// A context still being described.
pub struct Context {
    runner: Arc<dyn Runner>,
    engine: Arc<dyn Engine>,
    clock: Arc<dyn Clock>,
    filesystem: Arc<dyn FileSystem>,
    images: Option<Arc<dyn Images>>,
    stack: Source,
    settings: Settings,
    environment: Environment,
}

/// A world that answers from scripts: programs succeed and say nothing, no containers
/// are running, the clock is stopped at today, and the filesystem is the real one.
#[must_use]
pub fn a_context() -> Context {
    Context {
        runner: Arc::new(Scripted(Ok(spoke("")))),
        engine: Arc::new(Reporting::absent()),
        clock: Stopped::today(),
        filesystem: Arc::new(lemonfiber_adapters::Disk),
        images: None,
        stack: repository_stack(),
        settings: Settings::default(),
        environment: Environment::MacOs,
    }
}

/// A world that is this machine: programs run, the engine is the local one, and the
/// clock and the filesystem are real.
#[must_use]
pub fn a_live_context() -> Context {
    Context {
        runner: Arc::new(lemonfiber_adapters::Local),
        engine: Arc::new(lemonfiber_adapters::Daemon::local()),
        clock: Arc::new(lemonfiber_adapters::System),
        ..a_context()
    }
}

impl Context {
    /// The runner its programs are spawned through.
    #[must_use]
    pub fn runner(mut self, runner: Arc<dyn Runner>) -> Self {
        self.runner = runner;
        self
    }

    /// The engine it reports containers through.
    #[must_use]
    pub fn engine(mut self, engine: Arc<dyn Engine>) -> Self {
        self.engine = engine;
        self
    }

    /// The clock it reads.
    #[must_use]
    pub fn clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// The filesystem it reads and writes.
    #[must_use]
    pub fn filesystem(mut self, filesystem: Arc<dyn FileSystem>) -> Self {
        self.filesystem = filesystem;
        self
    }

    /// What the engine says it has pulled, where a start would otherwise ask a real
    /// daemon what else holds the ports it wants.
    #[must_use]
    pub fn images(mut self, images: Arc<dyn Images>) -> Self {
        self.images = Some(images);
        self
    }

    /// The stack it reads.
    #[must_use]
    pub fn over(mut self, stack: Source) -> Self {
        self.stack = stack;
        self
    }

    /// The settings it runs under.
    #[must_use]
    pub fn settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    /// The platform it believes it is on.
    #[must_use]
    pub fn environment(mut self, environment: Environment) -> Self {
        self.environment = environment;
        self
    }

    /// The context itself, ready for `Ctx`'s own `with_*` chain.
    #[must_use]
    pub fn build(self) -> Ctx {
        let live = lemonfiber_adapters::live();
        let seams = lemonfiber_core::ports::seams::Seams {
            runner: self.runner,
            engine: self.engine,
            clock: self.clock,
            filesystem: self.filesystem,
            images: self.images.unwrap_or(live.images),
            ..live
        };
        Ctx::new(seams, self.stack, self.settings, self.environment)
    }
}
