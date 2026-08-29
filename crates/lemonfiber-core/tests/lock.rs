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

mod common;

use common::stack::project;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_core::app::{claimed, dispatch, released, Command, Ctx};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::filesystem::{
    Fault, FileSystem, FsKind, Identity, Ownership, StorageFacts,
};
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

    async fn describe(&self, _path: &Path) -> StorageFacts {
        StorageFacts {
            kind: FsKind::Linking("test".to_owned()),
            removable: false,
            available: 0,
            total: 0,
        }
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
        Arc::clone(files) as Arc<dyn FileSystem>,
        Source::External(project()),
        Settings {
            env_file: Some(PathBuf::from("/tmp/lemonfiber-lock-test/.env")),
            ..Settings::default()
        },
        Environment::MacOs,
    )
}

/// A context that keeps no settings, and so has nowhere to put a claim.
fn ctx_without_settings(files: &Arc<Remembering>) -> Ctx {
    Ctx::new(
        Arc::new(lemonfiber_fixtures::ports::Idle),
        Arc::new(Reporting::absent()),
        lemonfiber_fixtures::ports::Stopped::today(),
        Arc::clone(files) as Arc<dyn FileSystem>,
        Source::External(project()),
        Settings::default(),
        Environment::MacOs,
    )
}

/// The whole point: the second run is told no, rather than both of them going ahead
/// and leaving the stack in a state neither asked for.
#[tokio::test]
async fn the_second_run_is_refused_while_the_first_holds_the_stack() {
    let files = Arc::new(Remembering::default());
    let first = claimed(&ctx(&files)).await;
    assert!(first.is_ok(), "nothing held it, so the first run took it");

    let second = claimed(&ctx(&files)).await;

    let refused = second.err().map(|problem| problem.summary.clone());
    assert_eq!(
        refused
            .as_deref()
            .map(|said| said.contains("working on this stack")),
        Some(true),
        "the second run is told what is happening, not just that it cannot: {refused:?}"
    );
}

/// A switch stops some services and starts others, so it is a lifecycle operation
/// and the stack has to be claimed for it. Driven through `dispatch` rather than
/// through the switch directly, because what is being asserted is that the route a
/// surface takes claims — both surfaces reach a switch through this one.
#[tokio::test]
async fn a_switch_is_refused_while_another_run_holds_the_stack() {
    let files = Arc::new(Remembering::default());
    let held = claimed(&ctx(&files)).await;
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
            .map(|about| about.contains("working on this stack")),
        Some(true),
        "the switch is told the stack is held, rather than going on to move services: {said:?}"
    );
}

/// Giving it back is what makes the next run possible; a lock that is only ever
/// taken is a stack nobody can operate twice.
#[tokio::test]
async fn giving_the_stack_back_lets_the_next_run_have_it() {
    let files = Arc::new(Remembering::default());
    let held = claimed(&ctx(&files)).await;

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
        claimed(&ctx(&files)).await.is_ok(),
        "so the next run can have the stack"
    );
}

/// A refusal an operator cannot act on is worse than none, so it says which process
/// has it and how long that one has been going.
#[tokio::test]
async fn a_refusal_names_the_run_holding_it_and_how_long_it_has_been() {
    let files = Arc::new(Remembering::default());
    files
        .write(&lockfile(), &format!("41207\n{}", 1_786_967_988_u64))
        .await;

    let refused = claimed(&ctx(&files)).await.err();
    let said = refused.map(|problem| format!("{} {}", problem.summary, problem.meaning));

    assert_eq!(
        said.as_deref().map(|said| said.contains("pid 41207")),
        Some(true),
        "{said:?}"
    );
    assert_eq!(
        said.as_deref()
            .map(|said| said.contains("12 seconds after")),
        Some(true),
        "twelve seconds between the stamp and this clock: {said:?}"
    );
}

/// A claim written by something that did not finish writing it still has to produce
/// a sentence, rather than a sentence with a hole where the process should be.
#[tokio::test]
async fn a_claim_that_says_nothing_about_itself_still_reads_as_a_sentence() {
    let files = Arc::new(Remembering::default());
    files.write(&lockfile(), "").await;

    let refused = claimed(&ctx(&files)).await.err();
    let said = refused.map(|problem| format!("{} {}", problem.summary, problem.meaning));

    assert_eq!(
        said.as_deref()
            .map(|said| said.contains("pid") || said.contains("second")),
        Some(false),
        "neither is claimed when neither was recorded: {said:?}"
    );
}

/// A run that was killed leaves the stack claimed, and `--force` is how an operator
/// who knows that says so.
#[tokio::test]
async fn forcing_takes_the_stack_from_a_run_that_left_it_claimed() {
    let files = Arc::new(Remembering::default());
    files.write(&lockfile(), "41207\n1").await;

    assert!(
        claimed(&ctx(&files).forcing()).await.is_ok(),
        "forcing takes it"
    );
    assert!(
        claimed(&ctx(&files)).await.is_err(),
        "and having taken it, holds it"
    );
}

/// A rehearsal changes nothing, so it takes nothing away from a run that is changing
/// something — and being unable to rehearse during a real run would make the safe
/// command the awkward one.
#[tokio::test]
async fn a_rehearsal_claims_nothing_and_is_refused_nothing() {
    let files = Arc::new(Remembering::default());
    files.write(&lockfile(), "41207\n1").await;

    assert!(claimed(&ctx(&files).rehearsing()).await.is_ok());
    assert_eq!(
        files.at(&lockfile()).as_deref(),
        Some("41207\n1"),
        "and it did not disturb the claim that was there"
    );
}

/// A machine keeping no settings has nowhere to put a claim, which is a machine with
/// nothing to serialise rather than an error to report.
#[tokio::test]
async fn a_machine_that_keeps_no_settings_is_not_blocked_by_the_lock() {
    let files = Arc::new(Remembering::default());
    let held = claimed(&ctx_without_settings(&files)).await;

    assert!(held.is_ok());
    if let Ok(claim) = held {
        released(&ctx_without_settings(&files), claim).await;
    }
    assert!(files.at(&lockfile()).is_none(), "nothing was written");
}
