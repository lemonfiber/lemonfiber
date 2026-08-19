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
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use common::files::Files;
use common::Fake;
use lemonfiber_core::app::bundle::{
    collect, write, Wanted, BUNDLE_LEAK, BUNDLE_NO_ROOM, BUNDLE_UNWRITTEN,
};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::backup::{Existing, Item, Manifest as BackupManifest};
use lemonfiber_core::bundle::{Contents, Piece, Taken, Terms, MANIFEST};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::archive::{Archive, Fault as ArchiveFault, Space};
use lemonfiber_core::ports::docker::{
    Container, Engine, ExecOutput, Failure as EngineFailure, Health, Lifecycle, LogLine, LogQuery,
    Stats, Stream,
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

/// An engine with nothing running, and one that cannot be reached at all.
struct Engine1(bool);

#[async_trait]
impl Engine for Engine1 {
    async fn list(&self, _project: &str) -> Result<Vec<Container>, EngineFailure> {
        if self.0 {
            return Ok(vec![Container {
                id: "abc".to_owned(),
                project: "media-stack".to_owned(),
                service: "sonarr".to_owned(),
                lifecycle: Lifecycle::Running,
                health: Health::Healthy,
                exit: None,
            }]);
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
        if !self.0 {
            return Err(EngineFailure::Unreachable {
                reason: "no engine here".to_owned(),
            });
        }
        let (sending, receiving) = tokio::sync::mpsc::channel(4);
        // A line with a key riding in a query string, which is the shape the whole
        // free-text rule exists for and the one nobody spots by eye.
        let _ = sending
            .send(LogLine {
                service: "prowlarr".to_owned(),
                stream: Stream::Stdout,
                at: None,
                line: format!(
                    "GET https://indexer.example.com/api?apikey={}&t=search",
                    key_shaped()
                ),
            })
            .await;
        Ok(receiving)
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
        configuration.map_or_else(Files::empty, Files::anywhere),
        stack,
        Settings {
            env_file: Some(PathBuf::from("/tmp/lemonfiber-bundle-test/.env")),
            stack_dir: Some(PathBuf::from("/tmp/lemonfiber-bundle-test")),
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
    let contents = collect(&context, LEMONFIBER, &Wanted::default())
        .await
        .unwrap_or_default();

    let names: Vec<&str> = contents
        .pieces
        .iter()
        .map(|piece| piece.name.as_str())
        .collect();
    assert!(names.contains(&"diagnosis.txt"));
    assert!(names.contains(&"services.txt"));
    assert!(names.contains(&"platform.txt"));
    assert!(names.contains(&"configuration.env"));
    // What the engine is running is read too, not merely asked for.
    assert!(contents
        .pieces
        .iter()
        .any(|piece| piece.name == "services.txt" && piece.body.contains("sonarr")));

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

/// The logs go in bounded and stated, and a key riding in one of them does not.
///
/// A log line is where the credential nobody spotted actually lives — inside a query
/// string, in text with no field name in front of it for an allow-list to recognise — so
/// this is the piece the free-text rule exists for.
#[tokio::test]
async fn a_bundle_holds_the_recent_logs_with_the_keys_in_them_replaced() {
    let context = ctx(Source::External(project()), true, None);
    let wanted = Wanted {
        lines: 5,
        ..Wanted::default()
    };
    let contents = collect(&context, LEMONFIBER, &wanted)
        .await
        .unwrap_or_default();

    let logs = contents
        .pieces
        .iter()
        .find(|piece| piece.name == "logs.txt")
        .map(|piece| piece.body.clone())
        .unwrap_or_default();
    assert!(logs.contains("https://indexer.example.com/api?"), "{logs}");
    assert!(
        !logs.contains(&key_shaped()),
        "the key does not ride out: {logs}"
    );

    // Stated, because an extract that does not say what it is an extract of reads as the
    // whole story — and the window is the operator's, not a fixed one.
    assert!(contents
        .manifest()
        .contains("the last 5 lines of each service"));
}

/// A value shaped the way a generated key is, built from character ranges rather than
/// written for the reason every credential-shaped fixture here is built.
fn key_shaped() -> String {
    ('a'..='j').chain('0'..='9').cycle().take(32).collect()
}

/// The case a bundle exists for: a machine where nothing answers. What can be read is
/// collected, and what cannot is named — because a gap nobody mentions reads as an
/// absence of trouble.
#[tokio::test]
async fn a_bundle_from_a_broken_machine_names_what_it_could_not_read() {
    let context = ctx(Source::External(Path::new("/no/such/stack")), false, None);
    let contents = collect(&context, LEMONFIBER, &Wanted::default())
        .await
        .unwrap_or_default();

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
    assert!(collect(&context, LEMONFIBER, &Wanted::default())
        .await
        .is_none());
}

/// An archive that records what it was asked to write, and can be told to have no room or
/// to fail outright.
struct Recorder {
    available: u64,
    fails: bool,
    /// Whether the room can be read at all. A machine that will not say how much is free
    /// is not a machine to refuse a bundle over — the bundle is the thing that explains it.
    measurable: bool,
    written: Mutex<Vec<Wrote>>,
}

/// One call to the writer, kept so a test can say what was asked for.
type Wrote = (PathBuf, Vec<(String, String)>);

impl Recorder {
    fn new(available: u64, fails: bool) -> Self {
        Self {
            available,
            fails,
            measurable: true,
            written: Mutex::new(Vec::new()),
        }
    }

    /// A recorder that cannot say how much room there is.
    fn unmeasurable() -> Self {
        Self {
            measurable: false,
            ..Self::new(0, false)
        }
    }

    fn wrote(&self) -> usize {
        self.written.lock().map_or(0, |written| written.len())
    }
}

#[async_trait]
impl Archive for Recorder {
    async fn space(&self, _dir: &Path, _items: &[Item]) -> Result<Space, ArchiveFault> {
        if self.measurable {
            return Ok(Space {
                needed: 0,
                available: self.available,
            });
        }
        Err(ArchiveFault::new("the volume would not say"))
    }
    async fn write(
        &self,
        _dest: &Path,
        _manifest: &BackupManifest,
        _items: &[Item],
    ) -> Result<(), ArchiveFault> {
        Err(ArchiveFault::new("unused"))
    }
    async fn write_files(
        &self,
        dest: &Path,
        files: &[(String, String)],
    ) -> Result<(), ArchiveFault> {
        if self.fails {
            return Err(ArchiveFault::new("the disk said no"));
        }
        if let Ok(mut written) = self.written.lock() {
            written.push((dest.to_path_buf(), files.to_vec()));
        }
        Ok(())
    }
    async fn existing(&self, _dir: &Path) -> Result<Vec<Existing>, ArchiveFault> {
        Ok(Vec::new())
    }
    async fn remove(&self, _dir: &Path, _name: &str) -> Result<(), ArchiveFault> {
        Ok(())
    }
}

/// Contents holding exactly what they are given.
fn holding(name: &str, body: String) -> Contents {
    Contents {
        pieces: vec![Piece {
            name: name.to_owned(),
            body,
        }],
        missing: Vec::new(),
        taken: Taken {
            lemonfiber: LEMONFIBER.to_owned(),
            stack: "1.0.0".to_owned(),
            at: "2026-08-17T12:00:00".to_owned(),
        },
        terms: Terms::default(),
    }
}

/// Somewhere to write that nothing will actually be written to — the archive is a fake,
/// and what is being proved is what it was asked for.
fn dest() -> PathBuf {
    PathBuf::from("/tmp/lemonfiber-bundle-test/support.tar.gz")
}

/// A bundle that holds nothing alarming is written, and what comes back is what the
/// operator needs before they attach it to anything: where it is, how big, and what is in
/// it — in that order, because the last is the one they should read first.
#[tokio::test]
async fn a_clean_bundle_is_written_and_says_what_it_holds() {
    let archive = Recorder::new(u64::MAX / 2, false);
    let contents = holding("configuration.env", "PUID=1000".to_owned());
    let written = write(&archive, &contents, &dest()).await;

    assert!(written.is_ok());
    assert_eq!(archive.wrote(), 1);
    let holds = written.map(|written| written.holds).unwrap_or_default();
    assert_eq!(holds.first().map(String::as_str), Some(MANIFEST));
    assert!(holds.iter().any(|name| name == "configuration.env"));
}

/// The one failure this whole feature exists to prevent. Nothing is written, and the file
/// that produced it is named, because an operator cannot fix what nobody points at.
#[tokio::test]
async fn a_bundle_that_would_leak_is_not_written_at_all() {
    let archive = Recorder::new(u64::MAX / 2, false);
    let contents = holding(
        "sonarr/config.xml",
        format!("<ApiKey>{}</ApiKey>", key_shaped()),
    );
    let refused = write(&archive, &contents, &dest()).await;

    assert_eq!(archive.wrote(), 0, "nothing may be written after a hit");
    assert!(refused.is_err_and(|problem| problem.code == BUNDLE_LEAK
        && problem
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("sonarr/config.xml"))));
}

/// A machine an operator is already asking for help about is not helped by having its disk
/// filled, so the room is read before anything is written rather than partway through.
#[tokio::test]
async fn a_bundle_with_nowhere_to_fit_is_refused_before_it_is_written() {
    let archive = Recorder::new(1024, false);
    let contents = holding("configuration.env", "PUID=1000".to_owned());
    let refused = write(&archive, &contents, &dest()).await;

    assert_eq!(archive.wrote(), 0);
    assert!(refused.is_err_and(|problem| problem.code == BUNDLE_NO_ROOM));
}

/// A bundle is written whole or not at all, so a write that fails leaves nothing to
/// mistake for one — and says where it was trying to write.
#[tokio::test]
async fn a_bundle_that_cannot_be_written_says_so_and_says_where() {
    let archive = Recorder::new(u64::MAX / 2, true);
    let contents = holding("configuration.env", "PUID=1000".to_owned());
    let refused = write(&archive, &contents, &dest()).await;

    assert!(
        refused.is_err_and(|problem| problem.code == BUNDLE_UNWRITTEN
            && problem
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("support.tar.gz")))
    );
}

/// A machine that will not say how much room it has is not one to refuse a bundle over.
/// The bundle is the thing that would explain why it will not say.
#[tokio::test]
async fn a_bundle_is_still_written_where_the_room_cannot_be_read() {
    let archive = Recorder::unmeasurable();
    let contents = holding("configuration.env", "PUID=1000".to_owned());

    assert!(write(&archive, &contents, &dest()).await.is_ok());
    assert_eq!(archive.wrote(), 1);
}
