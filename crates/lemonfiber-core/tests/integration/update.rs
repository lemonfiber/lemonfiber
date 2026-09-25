//! Moving the stack onto the image versions this build pins.
//!
//! Driven through `dispatch` as every surface reaches it, and from here rather than
//! from a `#[cfg(test)]` module because the confirmed half is `async` end to end — an
//! async path exercised only in-crate has its coverage counted from the copy that
//! never ran.
//!
//! **The engine and the runner are one machine in two halves.** A staged update stops
//! the stack, captures it while nothing can be writing, and then starts one service at
//! a time — so the engine has to say "nothing is running" while the capture is taken
//! and "this one is" immediately afterwards. Two fakes answering independently could
//! not be both, and one answering by how many times it had been asked would be
//! answering a question about this file rather than about the run.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_core::app::{Command, Ctx, Outcome, Waiting};
use lemonfiber_core::archive::{Archive, Archiving, Fault as ArchiveFault, Reader, Space, Vault};
use lemonfiber_core::backup::{Existing, Item, Manifest as BackupManifest};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::Settings;
use lemonfiber_core::ports::docker::{
    Container, Engine, ExecOutput, Failure as EngineFailure, Health, Image, Lifecycle, LogLine,
    LogQuery, Stats,
};
use lemonfiber_core::ports::process::{Failure as RunFailure, Output, Runner};
use lemonfiber_core::update::run::Asked;
use lemonfiber_core::update::{Applied, Ending, Reversal};
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::spoke;
use tokio::sync::mpsc::{channel, Receiver};

/// The version the manifest pins Sonarr at, and the one behind it.
const SONARR: (&str, &str) = ("4.0.14", "4.0.15");

/// How a service the run started comes back, once it has been started.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Coming {
    /// Running and answering its own probe.
    Answering,
    /// Running and saying it is not working.
    Unwell,
    /// Nothing appears at all, so the wait runs out.
    Never,
    /// Still inside its probe's start period for this many listings, then answering.
    Slowly(usize),
    /// The engine stops answering the moment something has been started.
    Silent,
}

/// The service one Compose invocation names, where it names one.
///
/// The fence is what tells a start aimed at one service from the whole-stack stop in
/// front of it and the whole-stack start behind it: only a narrowed invocation puts
/// names after `--`.
fn fenced(argv: &[String]) -> Option<String> {
    let at = argv.iter().position(|word| word == "--")?;
    argv.get(at + 1).cloned()
}

/// What the engine reports and what the runner did, kept in one place.
struct Machine {
    /// Every argument vector Compose was handed, in order.
    seen: Mutex<Vec<Vec<String>>>,
    /// The services started so far, in the order the run started them.
    started: Mutex<Vec<String>>,
    /// How many times the engine has been asked what is running.
    asked: Mutex<usize>,
    /// How a started service comes back.
    coming: Coming,
    /// What Compose says to a start, where a test is about it refusing one.
    start: Mutex<Option<Result<Output, RunFailure>>>,
    /// What Compose says to the first whole-stack invocation — the stop in front of
    /// the capture — where a test is about it refusing that.
    stack: Mutex<Option<Result<Output, RunFailure>>>,
    /// What Compose says to the whole-stack start behind the run, where a test is
    /// about it refusing that.
    bringing_back: Mutex<Option<Result<Output, RunFailure>>>,
    /// How many whole-stack invocations have been asked for.
    ///
    /// The stop in front of the capture is the first and the start that puts
    /// everything back is the last, so a test that wants one of them refused has to
    /// be able to say which. Refusing "the stack" without saying when would always
    /// refuse the stop, and the two leave the machine in opposite states.
    stack_actions: Mutex<usize>,
}

impl Machine {
    /// A machine whose started services come back the given way.
    fn coming(coming: Coming) -> Arc<Self> {
        Arc::new(Self {
            seen: Mutex::new(Vec::new()),
            started: Mutex::new(Vec::new()),
            asked: Mutex::new(0),
            coming,
            start: Mutex::new(None),
            stack: Mutex::new(None),
            bringing_back: Mutex::new(None),
            stack_actions: Mutex::new(0),
        })
    }

    /// The same machine, with Compose refusing the first start it is asked for.
    fn refusing(self: Arc<Self>, said: Result<Output, RunFailure>) -> Arc<Self> {
        if let Ok(mut start) = self.start.lock() {
            *start = Some(said);
        }
        self
    }

    /// The same machine, with Compose refusing the stop that comes before the capture.
    fn refusing_the_stack(self: Arc<Self>, said: Result<Output, RunFailure>) -> Arc<Self> {
        if let Ok(mut stack) = self.stack.lock() {
            *stack = Some(said);
        }
        self
    }

    /// The same machine, with Compose refusing the start that puts the stack back.
    ///
    /// The run has finished by then and every step of it has succeeded, so this is
    /// the stack failing to come back rather than the update failing.
    fn refusing_to_bring_it_back(self: Arc<Self>, said: Result<Output, RunFailure>) -> Arc<Self> {
        if let Ok(mut back) = self.bringing_back.lock() {
            *back = Some(said);
        }
        self
    }

    /// The last thing Compose was asked to do, as the words it was asked in.
    fn last(&self) -> Vec<String> {
        self.seen
            .lock()
            .ok()
            .and_then(|seen| seen.last().cloned())
            .unwrap_or_default()
    }

    /// The services this run has started, in order.
    fn started(&self) -> Vec<String> {
        self.started
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default()
    }

    /// One container for a service the run has started.
    fn container(service: &str, health: Health) -> Container {
        Container {
            id: format!("id-{service}"),
            project: "lemonfiber".to_owned(),
            service: service.to_owned(),
            lifecycle: Lifecycle::Running,
            health,
            published: Vec::new(),
            mounts: Vec::new(),
            exit: None,
        }
    }

    /// How the services started so far answer their probes on this listing.
    fn health(&self, listings: usize) -> Health {
        match self.coming {
            Coming::Unwell => Health::Unhealthy,
            Coming::Slowly(after) if listings <= after => Health::Starting,
            _ => Health::Healthy,
        }
    }
}

#[async_trait]
impl Runner for Machine {
    async fn run(&self, argv: &[String]) -> Result<Output, RunFailure> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(argv.to_vec());
        }
        let Some(service) = fenced(argv) else {
            let nth = self.stack_actions.lock().map_or(1, |mut asked| {
                *asked += 1;
                *asked
            });
            let refusal = if nth == 1 {
                &self.stack
            } else {
                &self.bringing_back
            };
            return refusal
                .lock()
                .ok()
                .and_then(|mut said| said.take())
                .unwrap_or_else(|| Ok(spoke("")));
        };
        if let Some(said) = self.start.lock().ok().and_then(|mut start| start.take()) {
            return said;
        }
        if let Ok(mut started) = self.started.lock() {
            started.push(service);
        }
        Ok(spoke(""))
    }
}

#[async_trait]
impl Engine for Machine {
    async fn list(&self, _project: &str) -> Result<Vec<Container>, EngineFailure> {
        let listings = self.asked.lock().map_or(0, |mut asked| {
            *asked += 1;
            *asked
        });
        let started = self.started();
        if started.is_empty() {
            return Ok(Vec::new());
        }
        if self.coming == Coming::Silent {
            return Err(EngineFailure::Unreachable {
                reason: "the daemon went away".to_owned(),
            });
        }
        if self.coming == Coming::Never {
            return Ok(Vec::new());
        }
        let health = self.health(listings);
        Ok(started
            .iter()
            .map(|service| Self::container(service, health))
            .collect())
    }

    async fn exec(&self, container: &str, _argv: &[String]) -> Result<ExecOutput, EngineFailure> {
        Err(EngineFailure::NoSuchContainer {
            name: container.to_owned(),
        })
    }

    async fn stats(&self, _project: &str) -> Result<Receiver<(String, Stats)>, EngineFailure> {
        let (_sender, receiver) = channel(1);
        Ok(receiver)
    }

    async fn logs(
        &self,
        _project: &str,
        _services: &[String],
        _query: LogQuery,
    ) -> Result<Receiver<LogLine>, EngineFailure> {
        let (_sender, receiver) = channel(1);
        Ok(receiver)
    }
}

/// An archive that writes wherever it is told and remembers that it did.
struct Kept {
    /// Whether writing works at all, which is the precondition an update rests on.
    writes: bool,
    /// Every destination it was asked to write, in order.
    wrote: Mutex<Vec<PathBuf>>,
}

impl Kept {
    /// An archive that writes, or one that will not.
    fn writing(writes: bool) -> Arc<Self> {
        Arc::new(Self {
            writes,
            wrote: Mutex::new(Vec::new()),
        })
    }

    /// How many archives it was asked to write.
    fn written(&self) -> usize {
        self.wrote.lock().map_or(0, |wrote| wrote.len())
    }
}

#[async_trait]
impl Archive for Kept {
    async fn space(&self, _dir: &Path, _items: &[Item]) -> Result<Space, ArchiveFault> {
        Ok(Space {
            needed: 0,
            available: 1 << 30,
        })
    }
    async fn write(
        &self,
        dest: &Path,
        _manifest: &BackupManifest,
        _items: &[Item],
    ) -> Result<(), ArchiveFault> {
        if !self.writes {
            return Err(ArchiveFault::new("the disk said no"));
        }
        if let Ok(mut wrote) = self.wrote.lock() {
            wrote.push(dest.to_path_buf());
        }
        Ok(())
    }
    async fn write_files(
        &self,
        _dest: &Path,
        _files: &[(String, String)],
    ) -> Result<(), ArchiveFault> {
        Err(ArchiveFault::new("an update writes no bundle"))
    }
    async fn existing(&self, _dir: &Path) -> Result<Vec<Existing>, ArchiveFault> {
        Ok(Vec::new())
    }
    async fn remove(&self, _dir: &Path, _name: &str) -> Result<(), ArchiveFault> {
        Ok(())
    }
}

/// The other half of the port, so one fake is the one adapter a run holds.
#[async_trait]
impl Reader for Kept {
    async fn read_manifest(&self, _src: &Path) -> Result<BackupManifest, ArchiveFault> {
        Err(ArchiveFault::new("an update never reads an archive back"))
    }
    async fn extract(
        &self,
        _src: &Path,
        _targets: &[(String, PathBuf)],
    ) -> Result<(), ArchiveFault> {
        Err(ArchiveFault::new("an update never reads an archive back"))
    }
}

/// One image this machine has pulled, standing on lemonfiber's own project.
fn pulled(image: &str, tag: &str) -> Image {
    Image {
        tags: vec![format!("{image}:{tag}")],
        bytes: 1,
        projects: vec!["lemonfiber".to_owned()],
    }
}

/// The images a stack standing behind its pins would report.
fn behind(services: &[(&str, &str)]) -> Vec<Image> {
    services
        .iter()
        .map(|(service, tag)| pulled(&format!("lscr.io/linuxserver/{service}"), tag))
        .collect()
}

/// A context over the stack this repository carries, against `machine`.
fn ctx(machine: &Arc<Machine>, images: Vec<Image>, archive: &Arc<Kept>) -> Ctx {
    lemonfiber_testing::a_context()
        .runner(Arc::clone(machine) as Arc<dyn Runner>)
        .engine(Arc::clone(machine) as Arc<dyn Engine>)
        .filesystem(Files::empty())
        .images(Pulled::holding(images))
        .build()
        .with_http(Fake::silent())
        .with_archives(Archiving {
            paths: Paths::rooted(Path::new("/cfg"), Path::new("/data")),
            vault: Arc::clone(archive) as Arc<dyn Vault>,
        })
        .with_patience(Duration::ZERO)
}

/// What was asked of an update: every service, agreed to or not, waiting or not.
fn asking(confirm: bool, wait: Waiting) -> Command {
    Command::Update(Asked {
        service: None,
        confirm,
        wait,
    })
}

/// The report a dispatched update produced, or nothing where it refused.
fn reported(
    outcome: Result<Outcome, Box<lemonfiber_core::error::Problem>>,
) -> Option<lemonfiber_core::update::run::Report> {
    match outcome {
        Ok(Outcome::Update(report)) => Some(report),
        _ => None,
    }
}

/// What one service came to, by name.
fn came_to(applied: &[Applied], service: &str) -> Option<(Ending, Reversal)> {
    applied
        .iter()
        .find(|one| one.service == service)
        .map(|one| (one.ending, one.reversal))
}

// ── What the run writes down ──────────────────────────────────────────────────

/// The same context, with somewhere to keep a record.
///
/// A whole layout is an environment file and a stack directory, and the contexts
/// above carry neither — so an update run against one has nowhere to write what it
/// did and writes nothing, which is what every case before this one is about.
fn recording(machine: &Arc<Machine>, archive: &Arc<Kept>, name: &str) -> (Ctx, PathBuf) {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("journal-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let stack = dir.join("data").join("stack");
    let _ = std::fs::create_dir_all(&stack);

    let context = lemonfiber_testing::a_context()
        .runner(Arc::clone(machine) as Arc<dyn Runner>)
        .engine(Arc::clone(machine) as Arc<dyn Engine>)
        .filesystem(Files::empty())
        .settings(Settings {
            env_file: Some(dir.join(".env")),
            stack_dir: Some(stack),
            ..Settings::default()
        })
        .build()
        .with_images(Pulled::holding(behind(&[("sonarr", SONARR.0)])))
        .with_http(Fake::silent())
        .with_archives(Archiving {
            paths: Paths::rooted(Path::new("/cfg"), Path::new("/data")),
            vault: Arc::clone(archive) as Arc<dyn Vault>,
        })
        .with_patience(Duration::ZERO);

    (context, dir.join("journal.jsonl"))
}

// ── What is still coming down ─────────────────────────────────────────────────

mod capturing;
mod checking;
mod finishing;
mod restarting;
