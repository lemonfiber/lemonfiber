//! One lifecycle operation on a stack at a time.
//!
//! From here rather than from a `#[cfg(test)]` module because claiming is `async`,
//! and an async path exercised only in-crate has its coverage counted from the copy
//! that never ran.
//!
//! The fake filesystem below deliberately does **not** implement `claim`. The port's
//! default — a look followed by a write — is what a fake should use, and leaving it
//! in place is what proves the default works; the atomic override that matters is the
//! real adapter's, and that one is tested against a real filesystem beside it.
//!
//! Every case about a wait runs on a paused clock. The wait is five minutes of real
//! time and six hundred looks, and a suite that sat through either would be a suite
//! nobody runs — so the runtime advances its own timers the moment nothing is ready,
//! and the whole bound is exercised in the time it takes to read a map six hundred
//! times.

mod common;

use common::stack::project;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_core::app::{claimed, dispatch, released, Command, Ctx};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::filesystem::{
    Fault, FileSystem, FsKind, Identity, Ownership, Storage, StorageFacts,
};
use lemonfiber_core::ports::Narrator;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::support::Reporting;

/// A filesystem that remembers what was written to it.
///
/// The shared fakes are all read-only — they answer with what a test seeded and
/// forget writes — and a lock is nothing but a write somebody else can see, so this
/// one keeps them.
#[derive(Default)]
struct Remembering {
    held: Mutex<HashMap<PathBuf, String>>,
}

impl Remembering {
    /// What is at this path, if anything.
    fn at(&self, path: &Path) -> Option<String> {
        self.held
            .lock()
            .ok()
            .and_then(|held| held.get(path).cloned())
    }
}

#[async_trait]
impl FileSystem for Remembering {
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf, Fault> {
        Ok(path.to_path_buf())
    }

    async fn touch(&self, _path: &Path) -> Result<(), Fault> {
        Ok(())
    }

    async fn link(&self, _from: &Path, _to: &Path) -> Result<(), Fault> {
        Ok(())
    }

    async fn identify(&self, _path: &Path) -> Result<Identity, Fault> {
        Err(Fault::new("unused"))
    }

    async fn remove(&self, path: &Path) {
        if let Ok(mut held) = self.held.lock() {
            held.remove(path);
        }
    }

    async fn read(&self, path: &Path) -> Option<String> {
        self.at(path)
    }

    async fn write(&self, path: &Path, contents: &str) {
        if let Ok(mut held) = self.held.lock() {
            held.insert(path.to_path_buf(), contents.to_owned());
        }
    }

    async fn ownership(&self, _path: &Path) -> Option<Ownership> {
        None
    }
}

#[async_trait]
impl Storage for Remembering {
    async fn describe(&self, _path: &Path) -> StorageFacts {
        StorageFacts {
            point: PathBuf::new(),
            kind: FsKind::Linking("test".to_owned()),
            removable: false,
            available: 0,
            total: 0,
        }
    }
}

/// Everything a wait said, in the order it said it.
#[derive(Default)]
struct Heard(tokio::sync::Mutex<Vec<String>>);

#[async_trait]
impl Narrator for Heard {
    async fn say(&self, said: &str) {
        self.0.lock().await.push(said.to_owned());
    }
}

impl Heard {
    /// Everything it has heard so far, joined so a case can ask one question of it.
    async fn said(&self) -> String {
        self.0.lock().await.join(" / ")
    }
}

/// Where a claim would go, given the settings below.
fn lockfile() -> PathBuf {
    PathBuf::from("/tmp/lemonfiber-lock-test/lifecycle.lock")
}

/// A context keeping its settings beside a scratch env file, on this filesystem.
fn ctx(files: &Arc<Remembering>) -> Ctx {
    Ctx::new(
        Arc::new(lemonfiber_fixtures::ports::Idle),
        Arc::new(Reporting::absent()),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: Arc::clone(files) as Arc<dyn FileSystem>,
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        Settings {
            env_file: Some(PathBuf::from("/tmp/lemonfiber-lock-test/.env")),
            ..Settings::default()
        },
        Environment::MacOs,
    )
}

/// The same context, saying what it is waiting for where a case can read it.
fn ctx_narrating(files: &Arc<Remembering>, heard: &Arc<Heard>) -> Ctx {
    ctx(files).narrating(Arc::clone(heard) as Arc<dyn Narrator>)
}

/// A context that keeps no settings, and so has nowhere to put a claim.
fn ctx_without_settings(files: &Arc<Remembering>) -> Ctx {
    Ctx::new(
        Arc::new(lemonfiber_fixtures::ports::Idle),
        Arc::new(Reporting::absent()),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: Arc::clone(files) as Arc<dyn FileSystem>,
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        Settings::default(),
        Environment::MacOs,
    )
}

/// A claim written by somebody else, as the file records one.
///
/// Assembled here rather than by claiming, because what is being driven is a run
/// arriving to find the stack already taken — and the pid has to be one this process
/// does not answer to for that to be what it is.
fn written(pid: u32, since: u64, doing: &str) -> String {
    format!("{pid}\n{since}\n{doing}")
}

/// A pid this process cannot have, so a claim carrying it is unambiguously somebody
/// else's.
///
/// Derived rather than written down: a literal would be this run's own pid on the
/// machine unlucky enough to be given it, and that machine would see the one
/// assertion here that is about telling the two apart fail for a reason nothing in
/// the file explains.
fn somebody_else() -> u32 {
    std::process::id().wrapping_add(1)
}

/// The moment the frozen clock in these contexts reads, twelve seconds before now.
const TWELVE_SECONDS_AGO: u64 = 1_790_812_788;

/// The whole point: the second operation happens, late, rather than not at all.
///
/// Two browser tabs half a second apart are the case this is for. Turning one of them
/// away makes the operator notice a refusal, work out that it was about timing rather
/// than about what they asked for, and ask again — where waiting makes it their turn
/// and then does what they asked.
#[tokio::test(start_paused = true)]
async fn the_second_operation_waits_for_its_turn_rather_than_being_turned_away() {
    let files = Arc::new(Remembering::default());
    let heard = Arc::new(Heard::default());
    let first = claimed(&ctx(&files), "up").await.ok();
    assert!(first.is_some(), "nothing held it, so the first run took it");

    // Both on this task rather than one of them spawned: the runtime advances its own
    // timers only when nothing is ready to run, and two futures joined here are idle
    // together at exactly the moments the wait depends on.
    let second_ctx = ctx_narrating(&files, &heard);
    let (second, ()) = tokio::join!(claimed(&second_ctx, "down"), async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        if let Some(claim) = first {
            released(&ctx(&files), claim).await;
        }
    });

    assert!(
        second.is_ok(),
        "the second operation took the stack once the first gave it back: {:?}",
        second.err().map(|problem| problem.summary.clone())
    );
    let said = heard.said().await;
    assert!(
        said.contains("already working on this stack (up)"),
        "it was told what it was waiting for: {said}"
    );
    assert!(
        said.contains("taking the stack now"),
        "and told when the waiting ended: {said}"
    );
}

/// A claim records what the run holding it is doing, because that is what the next
/// run to arrive is shown.
#[tokio::test]
async fn a_claim_records_what_the_run_holding_it_is_doing() {
    let files = Arc::new(Remembering::default());

    assert!(claimed(&ctx(&files), "up").await.is_ok());

    let marker = files.at(&lockfile()).unwrap_or_default();
    let lines: Vec<&str> = marker.lines().collect();
    assert_eq!(
        lines
            .first()
            .copied()
            .and_then(|pid| pid.parse::<u32>().ok()),
        Some(std::process::id()),
        "the process is still recorded, for whoever opens the file: {marker:?}"
    );
    assert_eq!(
        lines.last().copied(),
        Some("up"),
        "and so is the operation, for whoever has to wait behind it: {marker:?}"
    );
}

/// A wait that runs out is a refusal, because nothing here can tell a run that is
/// still going from one that was killed holding the stack.
#[tokio::test(start_paused = true)]
async fn a_run_whose_turn_never_comes_is_told_what_is_in_the_way() {
    let files = Arc::new(Remembering::default());
    files
        .write(
            &lockfile(),
            &written(somebody_else(), TWELVE_SECONDS_AGO, "up"),
        )
        .await;

    let refused = claimed(&ctx(&files), "down").await.err();
    let said = refused.map(|problem| {
        format!(
            "{} | {} | {}",
            problem.summary,
            problem.meaning,
            problem.detail.clone().unwrap_or_default()
        )
    });
    let said = said.unwrap_or_default();

    assert!(
        said.contains("another lemonfiber run is still working on this stack (up)"),
        "the operation in the way is named, and whose it is: {said}"
    );
    assert!(
        said.contains("12 seconds after that one started"),
        "twelve seconds between the stamp and this clock: {said}"
    );
    assert!(
        said.contains("waited 300 seconds for its turn"),
        "a refusal five minutes after the command says why it took five minutes: {said}"
    );
    assert!(
        said.contains(&format!("written by process {}", somebody_else())),
        "and the process is in the detail, where `--force` is: {said}"
    );
}

/// The case a pid was always the wrong noun for.
///
/// Two browser tabs on one server share a process, so a message naming the pid that
/// holds the stack names the reader's own run back at them. What is in the way is
/// another client, and nothing about it is the operator's to kill.
#[tokio::test(start_paused = true)]
async fn a_refusal_about_this_server_does_not_hand_back_the_readers_own_process() {
    let files = Arc::new(Remembering::default());
    files
        .write(
            &lockfile(),
            &written(std::process::id(), TWELVE_SECONDS_AGO, "up"),
        )
        .await;

    let refused = claimed(&ctx(&files), "down").await.err();
    let said = refused.map(|problem| format!("{} | {}", problem.summary, problem.meaning));
    let said = said.unwrap_or_default();

    assert!(
        said.contains("another client of this server is still working on this stack (up)"),
        "the other tab is another client, not another lemonfiber: {said}"
    );
    assert!(
        !said.contains("pid"),
        "and the reader is not handed their own process id: {said}"
    );
}

/// A claim from a copy that recorded two lines still has to produce a sentence,
/// rather than a sentence with a hole where the operation should be.
#[tokio::test(start_paused = true)]
async fn a_claim_that_named_no_operation_still_reads_as_a_sentence() {
    let files = Arc::new(Remembering::default());
    files
        .write(
            &lockfile(),
            &format!("{}\n{TWELVE_SECONDS_AGO}", somebody_else()),
        )
        .await;

    let refused = claimed(&ctx(&files), "down").await.err();
    let said = refused.map(|problem| format!("{} | {}", problem.summary, problem.meaning));
    let said = said.unwrap_or_default();

    assert!(
        said.contains("is still working on this stack |"),
        "no operation is invented for one that was never recorded: {said}"
    );
    assert!(
        said.contains("12 seconds after that one started"),
        "the age it did record is still claimed: {said}"
    );
}

/// A claim written by something that did not finish writing it still has to produce
/// a sentence, rather than one with three holes in it.
#[tokio::test(start_paused = true)]
async fn a_claim_that_says_nothing_about_itself_still_reads_as_a_sentence() {
    let files = Arc::new(Remembering::default());
    files.write(&lockfile(), "").await;
    let heard = Arc::new(Heard::default());

    let refused = claimed(&ctx_narrating(&files, &heard), "down").await.err();
    let said = refused.map(|problem| {
        format!(
            "{} | {} | {}",
            problem.summary,
            problem.meaning,
            problem.detail.clone().unwrap_or_default()
        )
    });
    let said = said.unwrap_or_default();

    assert!(
        said.contains("another lemonfiber run is still working on this stack |"),
        "nothing is claimed about an operation that was never recorded: {said}"
    );
    assert!(
        !said.contains("after that one started"),
        "nor about when it started: {said}"
    );
    assert!(
        !said.contains("written by process"),
        "nor about which process wrote it: {said}"
    );
    assert!(
        heard
            .said()
            .await
            .contains("another lemonfiber run is already working on this stack —"),
        "and the wait said as much while it waited: {}",
        heard.said().await
    );
}

/// A switch stops some services and starts others, so it is a lifecycle operation
/// and the stack has to be claimed for it. Driven through `dispatch` rather than
/// through the switch directly, because what is being asserted is that the route a
/// surface takes claims — both surfaces reach a switch through this one.
#[tokio::test(start_paused = true)]
async fn a_switch_waits_for_the_stack_and_is_refused_when_the_wait_runs_out() {
    let files = Arc::new(Remembering::default());
    let held = claimed(&ctx(&files), "up").await;
    assert!(held.is_ok(), "nothing held it, so the first run took it");

    let refused = dispatch(
        Command::Switch {
            forms: vec!["library".to_owned()],
        },
        &ctx(&files),
    )
    .await;

    let said = refused.err().map(|problem| problem.summary.clone());
    assert_eq!(
        said.as_deref()
            .map(|about| about.contains("working on this stack (up)")),
        Some(true),
        "the switch waited and then said what it waited for, rather than going on to \
         move services: {said:?}"
    );
}

/// Giving it back is what makes the next run possible; a lock that is only ever
/// taken is a stack nobody can operate twice.
#[tokio::test]
async fn giving_the_stack_back_lets_the_next_run_have_it() {
    let files = Arc::new(Remembering::default());
    let held = claimed(&ctx(&files), "up").await;

    assert!(held.is_ok());
    if let Ok(claim) = held {
        released(&ctx(&files), claim).await;
    }

    // Looked at before claiming again, because claiming again writes the marker
    // back and would hide whether releasing had removed it.
    assert!(
        files.at(&lockfile()).is_none(),
        "giving it back left nothing behind"
    );
    assert!(
        claimed(&ctx(&files), "down").await.is_ok(),
        "so the next run can have the stack"
    );
}

/// A run that was killed leaves the stack claimed for as long as the wait lasts, and
/// `--force` is how an operator who knows it is gone says so without sitting through
/// one.
#[tokio::test(start_paused = true)]
async fn forcing_takes_the_stack_without_waiting_for_a_run_that_is_gone() {
    let files = Arc::new(Remembering::default());
    files
        .write(&lockfile(), &written(somebody_else(), 1, "up"))
        .await;

    assert!(
        claimed(&ctx(&files).forcing(), "down").await.is_ok(),
        "forcing takes it"
    );
    assert!(
        claimed(&ctx(&files), "up").await.is_err(),
        "and having taken it, holds it"
    );
}

/// A rehearsal changes nothing, so it takes nothing away from a run that is changing
/// something — and being unable to rehearse during a real run would make the safe
/// command the awkward one.
#[tokio::test]
async fn a_rehearsal_claims_nothing_and_waits_for_nothing() {
    let files = Arc::new(Remembering::default());
    let marker = written(somebody_else(), 1, "up");
    files.write(&lockfile(), &marker).await;

    assert!(claimed(&ctx(&files).rehearsing(), "down").await.is_ok());
    assert_eq!(
        files.at(&lockfile()).as_deref(),
        Some(marker.as_str()),
        "and it did not disturb the claim that was there"
    );
}

/// A machine keeping no settings has nowhere to put a claim, which is a machine with
/// nothing to serialise rather than an error to report.
#[tokio::test]
async fn a_machine_that_keeps_no_settings_is_not_blocked_by_the_lock() {
    let files = Arc::new(Remembering::default());
    let held = claimed(&ctx_without_settings(&files), "up").await;

    assert!(held.is_ok());
    if let Ok(claim) = held {
        released(&ctx_without_settings(&files), claim).await;
    }
    assert!(files.at(&lockfile()).is_none(), "nothing was written");
}
