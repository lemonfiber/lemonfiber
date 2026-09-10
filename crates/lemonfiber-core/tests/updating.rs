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

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use common::stack::project;
use lemonfiber_core::app::update::Asked;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, Waiting};
use lemonfiber_core::archive::{Archive, Archiving, Fault as ArchiveFault, Reader, Space, Vault};
use lemonfiber_core::backup::{Existing, Item, Manifest as BackupManifest};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::{store, Protocols, Settings, QBITTORRENT_PASSWORD_KEY};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{
    Container, Engine, ExecOutput, Failure as EngineFailure, Health, Image, Lifecycle, LogLine,
    LogQuery, Stats,
};
use lemonfiber_core::ports::http::Http;
use lemonfiber_core::ports::process::{Failure as RunFailure, Output, Runner};
use lemonfiber_core::stack::Source;
use lemonfiber_core::update::{Applied, Ending, Reversal, State};
use lemonfiber_fixtures::downloads::{QBIT_FINISHED, QBIT_TORRENTS, SAB_EMPTY};
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::Stopped;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{a_password, spoke};
use tokio::sync::mpsc::{channel, Receiver};

/// The version the manifest pins Sonarr at, and the one behind it.
const SONARR: (&str, &str) = ("4.0.14", "4.0.15");

/// The same for Radarr, which the manifest declares after it.
const RADARR: (&str, &str) = ("5.13.0", "5.14.0");

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
    Ctx::new(
        Arc::clone(machine) as Arc<dyn Runner>,
        Arc::clone(machine) as Arc<dyn Engine>,
        Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: Files::empty(),
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        Settings::default(),
        Environment::MacOs,
    )
    .with_images(Pulled::holding(images))
    .with_http(Fake::silent())
    .keeping(Archiving {
        paths: Paths::rooted(Path::new("/cfg"), Path::new("/data")),
        vault: Arc::clone(archive) as Arc<dyn Vault>,
    })
    .waiting(Duration::ZERO)
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
) -> Option<lemonfiber_core::app::update::Report> {
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

#[tokio::test]
async fn a_bare_run_names_both_versions_and_changes_nothing() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(false, Waiting::Never), &context).await);

    let read = report.map(|report| {
        (
            report.state,
            report.confirmed,
            report.changes.len(),
            report
                .changes
                .first()
                .map(|change| (change.current.clone(), change.target.clone())),
        )
    });
    assert_eq!(
        read,
        Some((
            State::UpdatesAvailable,
            false,
            1,
            Some((SONARR.0.to_owned(), SONARR.1.to_owned()))
        ))
    );
    assert_eq!(archive.written(), 0, "a bare run captured something");
    assert!(machine.started().is_empty(), "a bare run started something");
}

/// A stack with nothing to move asks the download clients nothing.
///
/// Going to two clients to establish that a stack already on its pins would
/// interrupt nothing is this command making work out of an answer of "nothing".
#[tokio::test]
async fn a_stack_on_every_pin_asks_the_download_clients_nothing() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    let context = ctx(&machine, Vec::new(), &archive);

    let report = reported(dispatch(asking(false, Waiting::Never), &context).await);

    let read = report.map(|report| (report.state, report.in_flight.len(), report.confirmed));
    assert_eq!(read, Some((State::Current, 0, false)));
}

#[tokio::test]
async fn a_stack_already_on_its_pins_is_agreed_to_and_nothing_happens() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    let context = ctx(&machine, Vec::new(), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.map(|report| (report.state, report.confirmed, report.backup));
    assert_eq!(read, Some((State::Current, true, None)));
    assert_eq!(archive.written(), 0, "nothing to move was captured anyway");
}

#[tokio::test]
async fn a_pin_older_than_what_is_running_is_reported_and_never_attempted() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    // Standing on a version later than the pin, which is the one step lemonfiber
    // refuses: the database has been through it, and the older binary opening it is
    // what does the damage.
    let context = ctx(&machine, behind(&[("sonarr", "9.0.0")]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.map(|report| {
        (
            report.changes.first().map(|change| change.refused),
            report.applied.len(),
        )
    });
    assert_eq!(read, Some((Some(true), 0)));
    assert!(machine.started().is_empty(), "a refused step was taken");
}

#[tokio::test]
async fn a_confirmed_run_captures_before_anything_opens_its_state_on_the_new_image() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.map(|report| {
        (
            report.state,
            report.backup.is_some(),
            came_to(&report.applied, "sonarr"),
            report.halted,
        )
    });
    assert_eq!(
        read,
        Some((
            State::Updated,
            true,
            Some((Ending::Updated, Reversal::Restore)),
            None
        ))
    );
    assert_eq!(archive.written(), 1);
    assert_eq!(machine.started(), vec!["sonarr".to_owned()]);
    // The capture took the whole stack down, so the run puts it back — everything it
    // did not move is on the version it was already running.
    assert!(
        machine
            .last()
            .ends_with(&["up".to_owned(), "--detach".to_owned()]),
        "the stack was left down: {:?}",
        machine.last()
    );
}

#[tokio::test]
async fn a_capture_that_will_not_write_stops_the_run_before_anything_moves() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(false);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let refused = dispatch(asking(true, Waiting::Never), &context).await;

    assert!(refused.is_err(), "the update went ahead without a backup");
    assert!(
        machine.started().is_empty(),
        "a service was started with no archive to go back to"
    );
}

/// The capture is refused while anything might be writing, so the stack is stopped
/// before it is attempted — which means a capture that will not write leaves every
/// service down. That is not a thing the backup's own words would ever mention, and
/// it is the only part of this an operator has to act on straight away.
#[tokio::test]
async fn a_capture_that_will_not_write_says_the_stack_was_left_down() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(false);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let refused = dispatch(asking(true, Waiting::Never), &context).await;

    let problem = refused.err();
    assert_eq!(
        problem.as_ref().map(|one| one.code.to_string()).as_deref(),
        Some("UPDATE-4"),
        "the operator was told the capture failed and not that the stack is down"
    );
    let remedies: Vec<String> = problem
        .as_ref()
        .map(|one| {
            one.remedies
                .iter()
                .filter_map(|remedy| remedy.detail.clone())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        remedies
            .iter()
            .any(|detail| detail.contains("lemonfiber up")),
        "nothing offered to bring the stack back up: {remedies:?}"
    );
    assert!(
        problem.and_then(|one| one.cause).is_some(),
        "what stopped the capture was replaced rather than carried"
    );
}

/// A run where every step succeeded and the stack would not come back afterwards.
///
/// The update is done and there is nothing here to roll back. Reporting it as a
/// failure is what would have an operator reverse a database migration that worked,
/// which is the one move this whole feature exists to make unnecessary — so the
/// report survives, and where the stack was left is said in it.
#[tokio::test]
async fn a_stack_that_will_not_come_back_does_not_undo_the_update_it_reports() {
    let machine =
        Machine::coming(Coming::Answering).refusing_to_bring_it_back(Err(RunFailure::NotFound {
            program: "docker".to_owned(),
        }));
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.as_ref().map(|report| report.state);
    assert_eq!(
        read,
        Some(State::Updated),
        "a run whose every step succeeded was reported as the update failing"
    );
    assert_eq!(
        machine.started(),
        vec!["sonarr".to_owned()],
        "the service moved and answered its probe"
    );
    assert!(
        report
            .as_ref()
            .and_then(|report| report.backup.as_ref())
            .is_some(),
        "the backup taken before anything moved was dropped with the error"
    );
    let halted = report.and_then(|report| report.halted);
    assert!(
        halted
            .as_deref()
            .is_some_and(|why| why.contains("lemonfiber up")),
        "nothing said the stack is down or how to bring it back: {halted:?}"
    );
}

#[tokio::test]
async fn a_stack_that_will_not_come_down_is_never_captured_and_never_moved() {
    // The capture is refused while anything might be writing, so a stop that could
    // not even be run is the end of the run rather than something to go on past.
    let machine =
        Machine::coming(Coming::Answering).refusing_the_stack(Err(RunFailure::NotFound {
            program: "docker".to_owned(),
        }));
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let refused = dispatch(asking(true, Waiting::Never), &context).await;

    assert!(
        refused.is_err(),
        "the run went on over a stack it could not stop"
    );
    assert_eq!(
        archive.written(),
        0,
        "a stack that never stopped was captured"
    );
    assert!(machine.started().is_empty(), "a service was moved anyway");
}

#[tokio::test]
async fn a_service_that_does_not_come_back_halts_the_run_and_says_what_did_not_move() {
    let machine = Machine::coming(Coming::Never);
    let archive = Kept::writing(true);
    let context = ctx(
        &machine,
        behind(&[("sonarr", SONARR.0), ("radarr", RADARR.0)]),
        &archive,
    );

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.map(|report| {
        (
            report.state,
            came_to(&report.applied, "sonarr"),
            came_to(&report.applied, "radarr"),
            report.halted.unwrap_or_default(),
        )
    });
    assert!(
        matches!(
            &read,
            Some((
                State::Failed,
                Some((Ending::NotStarted, Reversal::Restore)),
                Some((Ending::NotReached, Reversal::Rollback)),
                said,
            )) if said.contains("sonarr") && said.contains("lemonfiber up")
        ),
        "{read:?}"
    );
    assert_eq!(
        machine.started(),
        vec!["sonarr".to_owned()],
        "the run went on into the rest of the stack"
    );
    assert!(
        !machine
            .last()
            .ends_with(&["up".to_owned(), "--detach".to_owned()]),
        "a halted run started the rest of the stack anyway: {:?}",
        machine.last()
    );
}

#[tokio::test]
async fn a_service_that_comes_back_unwell_is_told_apart_from_one_that_never_came_back() {
    let machine = Machine::coming(Coming::Unwell);
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.map(|report| {
        (
            report.state,
            report
                .applied
                .first()
                .and_then(|one| one.detail.clone())
                .unwrap_or_default(),
        )
    });
    assert!(
        matches!(&read, Some((State::Failed, said)) if said.contains("not working")),
        "{read:?}"
    );
}

#[tokio::test]
async fn a_service_still_starting_is_asked_again_rather_than_written_off() {
    // Unsettled on the first two listings and answering on the third, which is what a
    // service that is genuinely starting looks like. Patience is the run's own, so the
    // wait has somewhere to go rather than expiring on the first look.
    let machine = Machine::coming(Coming::Slowly(2));
    let archive = Kept::writing(true);
    let context =
        ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive).waiting(Duration::from_secs(600));

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.map(|report| (report.state, came_to(&report.applied, "sonarr")));
    assert_eq!(
        read,
        Some((State::Updated, Some((Ending::Updated, Reversal::Restore)))),
        "waiting is the point: the answer changed while it waited"
    );
}

#[tokio::test]
async fn an_engine_that_stops_answering_mid_run_is_reported_rather_than_waited_out() {
    let machine = Machine::coming(Coming::Silent);
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let said = report
        .and_then(|report| report.applied.first().and_then(|one| one.detail.clone()))
        .unwrap_or_default();
    assert!(said.contains("stopped answering"), "{said}");
}

#[tokio::test]
async fn a_start_that_could_not_be_run_leaves_the_service_where_it_was() {
    let machine = Machine::coming(Coming::Answering).refusing(Err(RunFailure::NotFound {
        program: "docker".to_owned(),
    }));
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.map(|report| {
        (
            came_to(&report.applied, "sonarr"),
            report
                .applied
                .first()
                .and_then(|one| one.detail.clone())
                .unwrap_or_default(),
        )
    });
    assert!(
        matches!(
            &read,
            Some((Some((Ending::NotFetched, Reversal::Rollback)), said)) if said.contains("docker")
        ),
        "{read:?}"
    );
}

#[tokio::test]
async fn a_compose_that_refuses_a_start_is_reported_in_composes_own_words() {
    let machine = Machine::coming(Coming::Answering).refusing(Ok(Output {
        status: Some(1),
        stdout: String::new(),
        stderr: "no such image".to_owned(),
    }));
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let said = report
        .and_then(|report| report.applied.first().and_then(|one| one.detail.clone()))
        .unwrap_or_default();
    assert_eq!(said, "no such image");
}

#[tokio::test]
async fn a_refusal_that_went_to_the_other_stream_is_still_the_operators_only_account() {
    let machine = Machine::coming(Coming::Answering).refusing(Ok(Output {
        status: Some(1),
        stdout: "the compose file names no such service".to_owned(),
        stderr: String::new(),
    }));
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let said = report
        .and_then(|report| report.applied.first().and_then(|one| one.detail.clone()))
        .unwrap_or_default();
    assert_eq!(said, "the compose file names no such service");
}

// ── What is still coming down ─────────────────────────────────────────────────

/// A private environment file recording qBittorrent's password, at a scratch path
/// unique to this case so concurrent tests do not share one.
fn env_at(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-update-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(".env");
    assert!(
        store::set(&path, QBITTORRENT_PASSWORD_KEY, &a_password()).is_ok(),
        "the scratch environment file is written"
    );
    path
}

/// The same context, reaching the download clients over `http` and holding a key for
/// the one of them that needs one.
fn transferring(
    machine: &Arc<Machine>,
    archive: &Arc<Kept>,
    http: Arc<dyn Http>,
    name: &str,
) -> Ctx {
    Ctx::new(
        Arc::clone(machine) as Arc<dyn Runner>,
        Arc::clone(machine) as Arc<dyn Engine>,
        Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: Files::empty(),
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        Settings {
            protocols: Protocols::both(),
            env_file: Some(env_at(name)),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_images(Pulled::holding(behind(&[("sonarr", SONARR.0)])))
    .with_http(http)
    .keeping(Archiving {
        paths: Paths::rooted(Path::new("/cfg"), Path::new("/data")),
        vault: Arc::clone(archive) as Arc<dyn Vault>,
    })
    .waiting(Duration::ZERO)
}

/// A qBittorrent still working on something, answering the same way every time.
fn still_coming_down() -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        ("/auth/login", vec![Answer::reply(200, "Ok.")]),
        ("/torrents/info", vec![Answer::reply(200, QBIT_TORRENTS)]),
        ("", vec![Answer::reply(200, SAB_EMPTY)]),
    ])
}

#[tokio::test]
async fn a_run_that_would_interrupt_a_transfer_is_refused_rather_than_carried_out() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    let context = transferring(&machine, &archive, still_coming_down(), "refused");

    let refused = dispatch(asking(true, Waiting::Never), &context).await;

    let code = refused.err().map(|problem| problem.code.to_string());
    assert_eq!(code.as_deref(), Some("UPDATE-3"));
    assert_eq!(archive.written(), 0, "the stack was captured anyway");
    assert!(machine.started().is_empty(), "a service was moved anyway");
}

#[tokio::test(start_paused = true)]
async fn a_run_asked_to_wait_lets_what_is_coming_down_finish_first() {
    // Coming down twice and finished on the third look: something to wait for, a look
    // with no news, and an end.
    let http = Fake::by_path_in_turn(vec![
        ("/auth/login", vec![Answer::reply(200, "Ok.")]),
        (
            "/torrents/info",
            vec![
                Answer::reply(200, QBIT_TORRENTS),
                Answer::reply(200, QBIT_TORRENTS),
                Answer::reply(200, QBIT_FINISHED),
            ],
        ),
        ("", vec![Answer::reply(200, SAB_EMPTY)]),
    ]);
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    let context = transferring(&machine, &archive, http, "waited");

    let report = reported(dispatch(asking(true, Waiting::ForTheDownloads), &context).await);

    let read = report.map(|report| (report.state, report.in_flight.len()));
    assert_eq!(
        read,
        Some((State::Updated, 1)),
        "the wait was taken and what it was waiting on is still named"
    );
    assert_eq!(machine.started(), vec!["sonarr".to_owned()]);
}
