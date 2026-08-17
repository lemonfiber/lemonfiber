//! Gathering a support bundle, driven end to end against a stack that is not there.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the provider and
//! credential checks are: the collector drives `#[async_trait]` clients built on another,
//! which are compiled twice and whose coverage is counted from the wrong copy when they
//! are exercised in-crate.
//!
//! The machine these run against is deliberately a broken one — no engine, no
//! configuration, a stack that will not read — because that is when a bundle is wanted.
//! A collector that produced nothing without a complete picture would produce nothing
//! exactly when it is needed, so what these prove is that it collects what it can and
//! names what it could not.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use common::Fake;
use lemonfiber_core::app::bundle::collect;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::bundle::MANIFEST;
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{
    Container, Engine, ExecOutput, Failure as EngineFailure, LogLine, LogQuery, Stats,
};
use lemonfiber_core::ports::filesystem::{
    Fault, FileSystem, FsKind, Identity, Ownership, StorageFacts,
};
use lemonfiber_core::ports::process::{Failure as RunFailure, Output, Runner};
use lemonfiber_core::ports::random::Random;
use lemonfiber_core::ports::time::Clock;
use lemonfiber_core::stack::Source;
use tokio::sync::mpsc::Receiver;

/// The repository's own copy of the stack, so what is collected is what a real
/// installation would have rather than an invented shape.
fn project() -> &'static Path {
    static PROJECT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    PROJECT
        .get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/media-stack"))
}

/// The version this build calls itself, as the command would pass it in.
const LEMONFIBER: &str = "0.7.0-test";

/// A filesystem holding one configuration file and nothing else.
struct Files {
    configuration: Option<&'static str>,
}

#[async_trait]
impl FileSystem for Files {
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf, Fault> {
        Ok(path.to_path_buf())
    }
    async fn touch(&self, _path: &Path) -> Result<(), Fault> {
        Err(Fault::new("unused"))
    }
    async fn link(&self, _from: &Path, _to: &Path) -> Result<(), Fault> {
        Err(Fault::new("unused"))
    }
    async fn identify(&self, _path: &Path) -> Result<Identity, Fault> {
        Err(Fault::new("unused"))
    }
    async fn remove(&self, _path: &Path) {}
    async fn read(&self, _path: &Path) -> Option<String> {
        self.configuration.map(str::to_owned)
    }
    async fn write(&self, _path: &Path, _contents: &str) {}
    async fn ownership(&self, _path: &Path) -> Option<Ownership> {
        None
    }
    async fn describe(&self, _path: &Path) -> StorageFacts {
        StorageFacts {
            kind: FsKind::Linking("test".to_owned()),
            removable: false,
            available: 0,
            total: 0,
        }
    }
}

/// An engine with nothing running, and one that cannot be reached at all.
struct Engine1(bool);

#[async_trait]
impl Engine for Engine1 {
    async fn list(&self, _project: &str) -> Result<Vec<Container>, EngineFailure> {
        if self.0 {
            return Ok(Vec::new());
        }
        Err(EngineFailure::Unreachable {
            reason: "no engine here".to_owned(),
        })
    }
    async fn logs(
        &self,
        _project: &str,
        _services: &[String],
        _query: LogQuery,
    ) -> Result<Receiver<LogLine>, EngineFailure> {
        Err(EngineFailure::Unreachable {
            reason: "unused".to_owned(),
        })
    }
    async fn exec(&self, _container: &str, _argv: &[String]) -> Result<ExecOutput, EngineFailure> {
        Err(EngineFailure::Unreachable {
            reason: "unused".to_owned(),
        })
    }
    async fn stats(&self, _project: &str) -> Result<Receiver<(String, Stats)>, EngineFailure> {
        Err(EngineFailure::Unreachable {
            reason: "unused".to_owned(),
        })
    }
}

/// A runner that spawns nothing.
struct Idle;

#[async_trait]
impl Runner for Idle {
    async fn run(&self, _argv: &[String]) -> Result<Output, RunFailure> {
        Err(RunFailure::NotFound {
            program: "unused".to_owned(),
        })
    }
}

/// A clock stopped at a fixed moment, so a bundle's provenance is the same run to run.
struct StoppedClock;

#[async_trait]
impl Clock for StoppedClock {
    fn now(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_786_968_000)
    }
}

/// Randomness built rather than written, for the reason every credential-shaped fixture
/// in this repository is built.
struct Bytes;

impl Random for Bytes {
    fn bytes(&self, n: usize) -> Option<Vec<u8>> {
        Some(
            ('a'..='p')
                .map(|letter| letter as u8)
                .cycle()
                .take(n)
                .collect(),
        )
    }
}

/// A context over a stack that is there, an engine that is or is not, and a configuration
/// file that is or is not.
fn ctx(stack: Source, running: bool, configuration: Option<&'static str>) -> Ctx {
    Ctx::new(
        Arc::new(Idle),
        Arc::new(Engine1(running)),
        Arc::new(StoppedClock),
        Arc::new(Files { configuration }),
        stack,
        Settings {
            env_file: Some(PathBuf::from("/tmp/lemonfiber-bundle-test/.env")),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(Fake::silent())
    .with_random(Arc::new(Bytes))
}

/// What a bundle from a working-enough machine holds: its own first page, the diagnosis,
/// what the engine is running, what the machine is, and the configuration — with every
/// value not named safe replaced on the way in.
#[tokio::test]
async fn a_bundle_holds_what_could_be_read_and_says_where_it_came_from() {
    let context = ctx(
        Source::External(project()),
        true,
        Some("PUID=1000\nINDEXER_APIKEY=something-nobody-should-see"),
    );
    let contents = collect(&context, LEMONFIBER).await.unwrap_or_default();

    let names: Vec<&str> = contents
        .pieces
        .iter()
        .map(|piece| piece.name.as_str())
        .collect();
    assert!(names.contains(&"diagnosis.txt"));
    assert!(names.contains(&"services.txt"));
    assert!(names.contains(&"platform.txt"));
    assert!(names.contains(&"configuration.env"));

    // Provenance, so a bundle read next week is not mistaken for this week's.
    assert_eq!(contents.taken.lemonfiber, LEMONFIBER);
    assert_eq!(contents.taken.at, "2026-08-17T12:00:00");
    assert_ne!(contents.taken.stack, "unknown");

    // Redacted on the way in, not on the way out.
    let files = contents.files();
    let configuration = files
        .iter()
        .find(|(name, _)| name == "configuration.env")
        .map(|(_, body)| body.clone())
        .unwrap_or_default();
    assert!(configuration.contains("PUID=1000"));
    assert!(configuration.contains("INDEXER_APIKEY=<redacted:"));
    assert!(!configuration.contains("something-nobody-should-see"));

    // The bundle's own first page comes first, and names what it holds.
    assert_eq!(files.first().map(|(name, _)| name.as_str()), Some(MANIFEST));
    assert!(files
        .first()
        .is_some_and(|(_, body)| body.contains("configuration.env") && body.contains(LEMONFIBER)));
}

/// The case a bundle exists for: a machine where nothing answers. What can be read is
/// collected, and what cannot is named — because a gap nobody mentions reads as an
/// absence of trouble.
#[tokio::test]
async fn a_bundle_from_a_broken_machine_names_what_it_could_not_read() {
    let context = ctx(Source::External(Path::new("/no/such/stack")), false, None);
    let contents = collect(&context, LEMONFIBER).await.unwrap_or_default();

    assert!(contents
        .missing
        .iter()
        .any(|gap| gap.contains("stack description")));
    assert!(contents
        .missing
        .iter()
        .any(|gap| gap.contains("container engine")));
    assert!(contents
        .missing
        .iter()
        .any(|gap| gap.contains("configuration")));
    assert_eq!(contents.taken.stack, "unknown");

    // Still a bundle: what is local knowledge is still known.
    assert!(contents
        .pieces
        .iter()
        .any(|piece| piece.name == "platform.txt"));
    // And its first page says what is not in it.
    let manifest = contents.manifest();
    assert!(manifest.contains("Could not be read:"));
}

/// A stand-in anyone could reproduce is a way back to the value it stands for, and a
/// bundle is a thing people post in public — so a machine that cannot provide the
/// randomness gets no bundle rather than a guessable one.
#[tokio::test]
async fn no_randomness_means_no_bundle_at_all() {
    struct Nothing;

    impl Random for Nothing {
        fn bytes(&self, _n: usize) -> Option<Vec<u8>> {
            None
        }
    }

    let context = ctx(Source::External(project()), true, None).with_random(Arc::new(Nothing));
    assert!(collect(&context, LEMONFIBER).await.is_none());
}
