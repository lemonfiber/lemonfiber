//! The context a test drives a command through.
//!
//! A context is built from every seam, a stack, settings and a platform, of which a test
//! usually varies one or two. This settles the rest, from one of two starting points:
//! [`a_context`], a world that answers from scripts, and [`a_live_context`], one that
//! runs programs and reads the disk for real. What a test varies, it names; what it
//! leaves, it gets from here.
//!
//! The transport, the randomness and the other seams a context holds are not here,
//! because `Ctx` already takes those by name — `with_http`, `with_random` — and this
//! builds a real one to chain from.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Engine, EngineVolumes, Images, Networks};
use lemonfiber_core::ports::http::Http;
use lemonfiber_core::ports::process::Runner;
use lemonfiber_core::ports::{Clock, FileSystem};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::ports::Stopped;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};

/// The stack this build carries, written into a directory of its own as a stack directory
/// holds it, so a run that writes the project beside it never writes into the repository. Held one level
/// down, so what a test puts beside the stack stays the test's own too.
#[must_use]
pub fn repository_stack() -> Source {
    stack_apart(lemonfiber_fixtures::stack::directory_into)
}

/// The repository's stack written apart for one test without either guard among its
/// own services: a stack no guard moves in.
#[must_use]
pub fn unguarded_stack() -> Source {
    stack_apart(|at| {
        lemonfiber_fixtures::stack::directory_without(
            at,
            &super::guards::BOTH.map(|guard| guard.plugin),
        );
    })
}

/// The stack this build carries, compiled in as it is, without either guard among its
/// own services.
#[must_use]
pub fn unguarded_embedded() -> Source {
    static CARRIED: std::sync::LazyLock<lemonfiber_core::stack::Carried> =
        std::sync::LazyLock::new(|| lemonfiber_core::stack::Carried {
            manifest: Box::leak(
                lemonfiber_fixtures::stack::assembled_without(
                    &super::guards::BOTH.map(|guard| guard.plugin),
                )
                .into_boxed_str(),
            ),
            files: lemonfiber_core::stack::BUNDLED.files,
        });
    Source::Embedded(&CARRIED)
}

/// A stack directory of its own for one test, as `written` writes it.
pub fn stack_apart(written: impl FnOnce(&Path)) -> Source {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let copy = Scratch::new(&format!(
        "repository-stack-{}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
    .kept()
    .join("stack");
    written(&copy);
    Source::External(Box::leak(copy.into_boxed_path()))
}

/// What the engine a test leaves unnamed says when asked for its images.
const NO_ENGINE: &str = "no container engine is asked in a test";

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
    networks: Option<Arc<dyn Networks>>,
    engine_volumes: Option<Arc<dyn EngineVolumes>>,
    http: Option<Arc<dyn Http>>,
    stack: Source,
    settings: Settings,
    environment: Environment,
}

/// A world that answers from scripts: programs succeed and say nothing, no containers
/// are running, the clock is stopped at today, and the filesystem is the real one. The
/// engine's images, networks and volumes and every service are fakes, so a daemon down or
/// wedged on the machine running the tests changes nothing.
#[must_use]
pub fn a_context() -> Context {
    Context {
        runner: Arc::new(Scripted(Ok(spoke("")))),
        engine: Arc::new(Reporting::absent()),
        clock: Stopped::today(),
        filesystem: Arc::new(lemonfiber_adapters::Disk),
        images: None,
        networks: None,
        engine_volumes: None,
        http: None,
        stack: repository_stack(),
        settings: Settings::default(),
        environment: Environment::MacOs,
    }
}

/// A world that is this machine: programs run, the engine, its images and networks are
/// the local ones, services are reached over the network, and the clock and the
/// filesystem are real.
#[must_use]
pub fn a_live_context() -> Context {
    let live = lemonfiber_adapters::live();
    Context {
        runner: Arc::new(lemonfiber_adapters::Local),
        engine: Arc::new(lemonfiber_adapters::Daemon::local()),
        clock: Arc::new(lemonfiber_adapters::System),
        images: Some(live.images),
        networks: Some(live.networks),
        engine_volumes: Some(live.engine_volumes),
        http: Some(live.http),
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

    /// The networks the engine says the project holds, where a start would otherwise
    /// sweep a real daemon's.
    #[must_use]
    pub fn networks(mut self, networks: Arc<dyn Networks>) -> Self {
        self.networks = Some(networks);
        self
    }

    /// The volumes the engine says it keeps, where a start would otherwise ask the real
    /// one.
    #[must_use]
    pub fn engine_volumes(mut self, engine_volumes: Arc<dyn EngineVolumes>) -> Self {
        self.engine_volumes = Some(engine_volumes);
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
            images: self
                .images
                .unwrap_or_else(|| lemonfiber_fixtures::pulled::Pulled::unreachable(NO_ENGINE)),
            networks: self
                .networks
                .unwrap_or_else(|| lemonfiber_fixtures::networked::Networked::none()),
            engine_volumes: self
                .engine_volumes
                .unwrap_or_else(|| lemonfiber_fixtures::volumed::Volumed::none()),
            http: self
                .http
                .unwrap_or_else(|| lemonfiber_fixtures::http::Fake::silent()),
            resolver: lemonfiber_fixtures::ports::Resolving::anywhere(),
            registry: lemonfiber_fixtures::registered::Registered::unpublished(),
            program: lemonfiber_fixtures::proving::Proving::unreachable(),
            serving: lemonfiber_fixtures::serving::Serves::unreachable(),
            ..live
        };
        Ctx::new(seams, self.stack, self.settings, self.environment)
    }
}
