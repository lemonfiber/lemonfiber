//! What this machine keeps of lemonfiber's, and taking it off, through the
//! dispatcher.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the forms
//! listing is: the app layer is compiled twice, and a path exercised only in-crate
//! has its coverage counted from the copy that never ran.
//!
//! Nothing reaches a real filesystem. Removal is the one operation that cannot be
//! let out into one and still be a test, and it is also the one where what was
//! *asked for* matters as much as what came back — a run that removed the wrong
//! directory answers exactly as a run that removed the right one does. So the eraser
//! is a fake that records, and the assertions are on the paths it was handed.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::stack::project;

use lemonfiber_core::adapters::{Daemon, Disk, Local, System};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::archive::{Archive, Archiving, Fault as ArchiveFault, Reader, Space};
use lemonfiber_core::backup::{Existing, Item, Manifest};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::stack::Source;
use lemonfiber_core::stored::{Removal, Stored};
use lemonfiber_fixtures::erasing::Erasing;

/// An archive nothing here opens.
///
/// Every method refuses rather than answering plausibly: a test that reached one of
/// these has found something, and a helpful empty answer would hide it.
struct Unasked;

#[async_trait::async_trait]
impl Archive for Unasked {
    async fn space(&self, _dir: &Path, _items: &[Item]) -> Result<Space, ArchiveFault> {
        Err(ArchiveFault::new("unused"))
    }

    async fn write(
        &self,
        _dest: &Path,
        _manifest: &Manifest,
        _items: &[Item],
    ) -> Result<(), ArchiveFault> {
        Err(ArchiveFault::new("unused"))
    }

    async fn write_files(
        &self,
        _dest: &Path,
        _files: &[(String, String)],
    ) -> Result<(), ArchiveFault> {
        Err(ArchiveFault::new("unused"))
    }

    async fn existing(&self, _dir: &Path) -> Result<Vec<Existing>, ArchiveFault> {
        Err(ArchiveFault::new("unused"))
    }

    async fn remove(&self, _dir: &Path, _name: &str) -> Result<(), ArchiveFault> {
        Err(ArchiveFault::new("unused"))
    }
}

#[async_trait::async_trait]
impl Reader for Unasked {
    async fn read_manifest(&self, _src: &Path) -> Result<Manifest, ArchiveFault> {
        Err(ArchiveFault::new("unused"))
    }

    async fn extract(
        &self,
        _src: &Path,
        _targets: &[(String, PathBuf)],
    ) -> Result<(), ArchiveFault> {
        Err(ArchiveFault::new("unused"))
    }
}

/// The layout a test reasons about: two directories under a scratch root that
/// nothing here ever touches, because nothing here reaches a real filesystem.
fn layout() -> Paths {
    Paths::rooted(Path::new("/scratch/config"), Path::new("/scratch/data"))
}

fn ctx(eraser: Arc<Erasing>) -> Ctx {
    Ctx::new(
        Arc::new(Local),
        Arc::new(Daemon::local()),
        Arc::new(System),
        Arc::new(Disk),
        Source::External(project()),
        Settings::default(),
        Environment::MacOs,
    )
    // A vault this run never reaches. `Archiving` is where a context holds the layout,
    // and the layout is the whole of what these two commands read from it — so what is
    // beside it answers nothing and is asked nothing.
    .keeping(Archiving {
        paths: layout(),
        vault: Arc::new(Unasked),
    })
    .erasing(eraser)
}

async fn answered(command: Command, eraser: &Arc<Erasing>) -> Stored {
    match dispatch(command, &ctx(Arc::clone(eraser))).await {
        Ok(Outcome::Stored(report)) => report,
        other => unreachable!("the disclosure answers with itself: {other:?}"),
    }
}

/// What is kept, where, and why — the answer to the question an operator has every
/// right to ask of something running on their own machine.
#[tokio::test]
async fn everything_this_machine_keeps_is_named_with_where_it_is_and_why() {
    let eraser = Erasing::willing();
    let report = answered(Command::Stored, &eraser).await;

    assert!(report.kept.len() > 10, "{}", report.kept.len());
    for entry in &report.kept {
        assert!(entry.at.starts_with("/scratch/"), "{entry:?}");
        assert!(
            entry.why.split_whitespace().count() >= 8,
            "{} says nothing an operator could act on",
            entry.what
        );
    }
    assert_eq!(report.removal, Removal::NotAsked);
    assert!(eraser.asked().is_empty(), "a listing removed something");
}

/// The envelope a surface actually reads, rendered here as well as in the crate's
/// own tests — the kind and the serialisation are reached from two compilations of
/// the file that holds them, and have to run in both.
#[tokio::test]
async fn the_answer_carries_its_own_kind_and_serialises_field_for_field() {
    let eraser = Erasing::willing();
    for (command, state) in [
        (Command::Stored, "not-asked"),
        (Command::Forget { confirm: false }, "unconfirmed"),
    ] {
        let json = dispatch(command, &ctx(Arc::clone(&eraser)))
            .await
            .ok()
            .map(Outcome::envelope)
            .and_then(|envelope| envelope.to_json())
            .unwrap_or_default();

        assert!(json.contains(r#""kind":"stored""#), "{json}");
        assert!(json.contains(&format!(r#""state":"{state}""#)), "{json}");
        assert!(json.contains(r#""secret":true"#), "{json}");
    }
    assert!(
        eraser.asked().is_empty(),
        "neither of those removes anything"
    );
}

/// The library is the absence that would otherwise read as an oversight, so it is
/// named rather than left out.
#[tokio::test]
async fn what_is_not_lemonfibers_is_named_in_the_same_answer() {
    let eraser = Erasing::willing();
    let report = answered(Command::Stored, &eraser).await;

    let said = report
        .beside
        .iter()
        .map(|beside| format!("{} {}", beside.what, beside.why))
        .collect::<Vec<String>>()
        .join(" ");
    assert!(said.contains("library"), "{said}");
    assert!(said.contains("container"), "{said}");
}

/// Nothing goes without the agreement, and what is agreed to is the listing itself
/// rather than a summary of it.
#[tokio::test]
async fn an_unconfirmed_removal_lists_what_would_go_and_takes_nothing() {
    let eraser = Erasing::willing();
    let report = answered(Command::Forget { confirm: false }, &eraser).await;

    assert_eq!(report.removal, Removal::Unconfirmed);
    assert!(report.kept.len() > 10);
    assert!(eraser.asked().is_empty(), "nothing was agreed to");
}

/// The claim removal rests on: two directories, and everything the layout names
/// under one of them.
#[tokio::test]
async fn a_confirmed_removal_takes_the_two_directories_and_says_which() {
    let eraser = Erasing::willing();
    let report = answered(Command::Forget { confirm: true }, &eraser).await;

    assert_eq!(
        eraser.asked(),
        vec![
            PathBuf::from("/scratch/config/lemonfiber"),
            PathBuf::from("/scratch/data/lemonfiber"),
        ]
    );
    assert!(
        matches!(&report.removal, Removal::Done { gone, left }
            if gone.len() == 2 && left.is_empty()),
        "{:?}",
        report.removal
    );
}

/// A directory that would not go is named with what the machine said about it.
/// Being told everything was removed and finding one still there is being told
/// something false.
#[tokio::test]
async fn a_directory_that_will_not_go_is_named_rather_than_swallowed() {
    let eraser = Erasing::refusing("permission denied");
    let report = answered(Command::Forget { confirm: true }, &eraser).await;

    assert!(
        matches!(&report.removal, Removal::Done { gone, left }
            if gone.is_empty()
                && left.len() == 2
                && left.iter().all(|still| still.why == "permission denied")),
        "{:?}",
        report.removal
    );
}

/// A rehearsal changes nothing, which is what it means everywhere else here.
#[tokio::test]
async fn a_rehearsal_reports_what_would_go_and_removes_none_of_it() {
    let eraser = Erasing::willing();
    let report = match dispatch(
        Command::Forget { confirm: true },
        &ctx(Arc::clone(&eraser)).rehearsing(),
    )
    .await
    {
        Ok(Outcome::Stored(report)) => report,
        other => unreachable!("the disclosure answers with itself: {other:?}"),
    };

    assert_eq!(report.removal, Removal::Unconfirmed);
    assert!(eraser.asked().is_empty(), "a rehearsal removed something");
}

/// A run that cannot say where its own files go is refused rather than answered
/// about a directory it guessed at.
#[tokio::test]
async fn a_run_that_does_not_know_where_its_files_are_is_refused() {
    let bare = Ctx::new(
        Arc::new(Local),
        Arc::new(Daemon::local()),
        Arc::new(System),
        Arc::new(Disk),
        Source::External(project()),
        Settings::default(),
        Environment::MacOs,
    );

    for command in [Command::Stored, Command::Forget { confirm: true }] {
        let refused = dispatch(command, &bare).await;
        assert!(
            refused
                .err()
                .is_some_and(|problem| problem.code.as_str() == "KEPT-1"),
            "a run with nowhere to look answered anyway"
        );
    }
}
