//! The fixtures that name a type from this crate.
//!
//! Everything a test stands in for lives in `lemonfiber-fixtures`, where the crate's own
//! tests and its integration tests reach the same one. These two stayed because they name
//! `Source`, `Settings` and `Ctx` — all above the boundary that crate depends on.

use crate::stack::Source;

pub(crate) use lemonfiber_fixtures::support::*;

/// The stack this repository carries, read from disk.
pub(crate) fn stack() -> Source {
    Source::External(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}

/// A stack source pointing nowhere, for the tests about what happens when it does.
pub(crate) fn nowhere() -> Source {
    Source::External(std::path::Path::new("/lemonfiber/no/such/stack"))
}

/// The context a test drives the application through.
///
/// `Ctx::new` takes seven arguments, of which a test usually varies one. Spelling all
/// seven out at every call meant the one that mattered was the hardest thing to see, and
/// that adding an eighth would be a change in sixty-odd places. So the six a test does not
/// care about are settled here: programs succeed and say nothing, no containers are
/// running, the clock and the filesystem are the real ones, the stack is the one this
/// repository carries, and the settings are the defaults.
///
/// What a test does vary, it names. The transport, the filesystem and the randomness are
/// not here because [`Ctx`] already takes those by name — `with_http`, `with_filesystem`,
/// `with_random` — and this builds a real one to chain from.
pub(crate) struct Context {
    runner: std::sync::Arc<dyn crate::ports::process::Runner>,
    engine: std::sync::Arc<dyn crate::ports::docker::Engine>,
    stack: Source,
    settings: crate::config::Settings,
    environment: crate::platform::Environment,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            runner: std::sync::Arc::new(Scripted(Ok(spoke("")))),
            engine: std::sync::Arc::new(Reporting::absent()),
            stack: stack(),
            settings: crate::config::Settings::default(),
            environment: crate::platform::Environment::MacOs,
        }
    }
}

impl Context {
    /// The engine this context reports containers through.
    pub(crate) fn engine(
        mut self,
        engine: std::sync::Arc<dyn crate::ports::docker::Engine>,
    ) -> Self {
        self.engine = engine;
        self
    }

    /// The runner its programs are spawned through — for the tests about a program that
    /// says something in particular, or is not installed at all.
    pub(crate) fn runner(
        mut self,
        runner: std::sync::Arc<dyn crate::ports::process::Runner>,
    ) -> Self {
        self.runner = runner;
        self
    }

    /// The stack it reads, where the test is about a different one.
    pub(crate) fn over(mut self, stack: Source) -> Self {
        self.stack = stack;
        self
    }

    /// The settings it runs under.
    pub(crate) fn settings(mut self, settings: crate::config::Settings) -> Self {
        self.settings = settings;
        self
    }

    /// The platform it believes it is on — for the handful of tests whose subject is
    /// what differs between them.
    pub(crate) fn environment(mut self, environment: crate::platform::Environment) -> Self {
        self.environment = environment;
        self
    }

    /// The context itself, ready for [`Ctx`]'s own `with_*` chain.
    pub(crate) fn build(self) -> crate::app::Ctx {
        crate::app::Ctx::new(
            self.runner,
            self.engine,
            std::sync::Arc::new(crate::adapters::System),
            std::sync::Arc::new(crate::adapters::Disk),
            self.stack,
            self.settings,
            self.environment,
        )
    }
}

/// A context with every seam settled, to vary by name from there.
pub(crate) fn a_context() -> Context {
    Context::default()
}

/// A journal line for a setting written over nothing — a fresh file, so the prior value
/// is absent.
///
/// Three test modules each built this, because the journal's shape is what they assert
/// against and stating it independently is the point: a test that echoed how apply
/// computes a change would pass whatever apply did. Stating it once does not weaken that
/// — what matters is that the expectation is written out rather than derived, not that
/// it is written out three times.
pub(crate) fn a_fresh_write(key: &str, value: &str) -> crate::journal::Change {
    crate::journal::Change {
        at: "t".to_owned(),
        operation: "apply".to_owned(),
        target: ".env".to_owned(),
        kind: crate::journal::Kind::Set {
            key: key.to_owned(),
            previous: None,
            current: value.to_owned(),
        },
    }
}

/// A private env file recording qBittorrent's password, at a scratch path unique to
/// the test so concurrent tests do not share one.
///
/// Here rather than beside any one test because more than one needs a client that can
/// be logged into: the dashboard, to fill its transfers panel, and a teardown, to find
/// out what stopping would interrupt. It names `config::store`, which is why it is on
/// this side of the fixtures boundary.
pub(crate) fn env_at(name: &str, password: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-env-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(".env");
    assert!(
        crate::config::store::set(&path, crate::config::QBITTORRENT_PASSWORD_KEY, password).is_ok(),
        "the scratch env file is written"
    );
    path
}
