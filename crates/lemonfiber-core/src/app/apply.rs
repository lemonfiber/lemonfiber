//! Writing a reviewed setup to disk, recoverably.
//!
//! The wizard's one phase that is not read-only: it takes a reviewed set of
//! answers and lands them as configuration. It is arranged so that stopping
//! anywhere in it is recoverable rather than wedged — the lifecycle marker moves
//! to `applying` and is persisted before the first write, each write is journalled
//! before it is made, and only once every write is done does the marker move to
//! `applied`. A run that stops in between is found on the next start as a failed
//! apply, with the journal holding exactly what to unwind.
//!
//! Applied here are the environment settings, the operator's data directory, and
//! the stack Compose reads — everything the stack needs on disk before it starts.
//! What is reversible is journalled: the settings and the data directory the
//! operator would not want silently left behind. The stack is lemonfiber's own
//! regenerable output and is not, since the next apply simply rewrites it.
//!
//! Recovery here is between whole writes, not within one. Each file is written in
//! place, so a stop in the middle of a single write can still tear that one file;
//! making each write atomic — a temporary file renamed over the target — is a
//! hardening the shared writer will grow, and is called out where it bites.

use std::path::{Path, PathBuf};

use crate::config::paths::Paths;
use crate::config::store;
use crate::error::{Code, Diagnose, Problem, Remedy, Severity};
use crate::journal::{Change, Journal, Kind};
use crate::stack::{self, Source};
use crate::wizard::{Phase, Wizard};

/// Write a reviewed setup to disk, driving the lifecycle and recording each
/// reversible write so an interrupted run can be unwound.
///
/// The wizard must be at review — every applicable question answered and
/// confirmed — or there is nothing settled to apply. Everything lands under the
/// install `paths`: the settings in the environment file, the lifecycle marker and
/// the change journal beside it, and the `source` stack where Compose reads it. The
/// marker reaches `applying` on disk before the first write, and each journal entry
/// lands before the write it describes, so a stop at any point leaves a state the
/// next run can recover rather than one it must guess at. The `stamp` times the
/// journal entries, which the wizard has no clock to do itself.
///
/// # Errors
///
/// Returns a [`Problem`] where review has not been reached, where a file could not
/// be read or written, where the data directory could not be created, or where the
/// stack could not be materialised — leaving the marker at `applying` and the
/// journal holding what had been written, which the next run recovers from.
pub fn apply(
    wizard: &mut Wizard,
    paths: &Paths,
    source: Source,
    stamp: &str,
) -> Result<(), Box<Problem>> {
    if !wizard.transition(Phase::Applying) {
        return Err(Box::new(not_reviewed()));
    }
    // Every failure is boxed as one problem on the way out, because a `Problem` is
    // large beside the `()` this returns on success — so the writes themselves stay
    // a plain sequence, each stopping the rest.
    write(wizard, paths, source, stamp).map_err(|fault| Box::new(fault.problem()))
}

/// A failure part-way through applying, named finely enough to remedy: a file the
/// config store could not read or write, a data directory that could not be made,
/// or the stack that could not be written out. Carried so the one boxing site
/// above turns whichever it was into a problem, rather than each write growing its
/// own.
enum Fault {
    /// A configuration, progress, or journal file could not be read or written.
    Store(store::Failure),
    /// The operator's chosen data directory could not be created.
    DirNotMade {
        /// The directory that could not be made.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
    /// The stack could not be materialised where Compose reads it.
    Stack(stack::Failure),
}

impl Fault {
    /// The problem to report, in the words that fit what actually failed.
    fn problem(&self) -> Problem {
        match self {
            Self::Store(failure) => failure.problem(),
            Self::Stack(failure) => failure.problem(),
            Self::DirNotMade { path, reason } => Problem::new(
                DIR_NOT_MADE,
                Severity::Error,
                "The data directory could not be created",
                "lemonfiber makes the directory the library and downloads live under before it starts anything. Setup has stopped, and the next run recovers it.",
                Remedy::new("Check the location is on a writable disk and try again"),
            )
            .with_detail(format!("{}: {reason}", path.display())),
        }
    }
}

/// Perform the writes of an apply, stopping at the first that fails.
///
/// Ordered for recovery: the applying marker is persisted first, each change is
/// journalled before it is written, and the applied marker is persisted last — so
/// a stop at any point leaves the marker and journal a later run reads.
fn write(wizard: &mut Wizard, paths: &Paths, source: Source, stamp: &str) -> Result<(), Fault> {
    let (progress, env_file, journal) = (paths.setup_progress(), paths.env_file(), paths.journal());

    store::write(&progress, &rendered(wizard)).map_err(Fault::Store)?;

    // The whole plan is diffed against the file as it stands before any write, so
    // every recorded `previous` is what was really there. The plan's keys are all
    // distinct, so no write moves a `previous` out from under a later one.
    let before = store::read(&env_file).map_err(Fault::Store)?;
    let plan = wizard.plan();
    let changes = plan.changes(&before, stamp);

    let mut log = Journal::new();

    // The library and downloads live under the operator's chosen location, so it
    // must exist before anything mounts it — but only where it does not already.
    // A location that is already there is the operator's own (a library to adopt),
    // left untouched and unrecorded, so unwinding never removes it. The chosen
    // location is iterated rather than matched, so a reviewed wizard that always
    // has one needs no unreachable "no location" branch.
    for root in data_root(wizard).into_iter().filter(|root| !root.exists()) {
        // Making the leaf makes every missing ancestor with it, so each one is
        // recorded — parent before child, to be unwound child-first — and none
        // lemonfiber creates is left without a way back. Journalled before the
        // directories are made, so a stop between records paths that may not exist
        // yet, whose removal is a harmless no-op.
        for ancestor in made_by(&root) {
            log.record(made(&ancestor, stamp));
        }
        store::write(&journal, &lines(&log)).map_err(Fault::Store)?;
        std::fs::create_dir_all(&root).map_err(|err| Fault::DirNotMade {
            path: root,
            reason: err.to_string(),
        })?;
    }

    // The stack is written where Compose reads it. An embedded stack is
    // lemonfiber's own regenerable output — materialised the same way on every
    // run, holding nothing an operator could lose — so it is not journalled: its
    // undo is simply that the next apply rewrites it, and a directory left behind
    // is a build artifact, not stranded work. An external stack is the operator's,
    // already on disk, and materialising it writes nothing.
    source
        .materialise(Some(&paths.stack()))
        .map_err(Fault::Stack)?;

    // `changes` is one entry per setting in the same order, so each pairs with the
    // key and value it was built from; they are two views of the same list, walked
    // together.
    for (change, (key, value)) in changes.into_iter().zip(plan.settings()) {
        // Journalled before it is written: a run that dies between the two leaves a
        // record of a change that may not have landed, and undoing that restores
        // what was already there — harmless. The reverse would leave a real write
        // with nothing to unwind it.
        log.record(change);
        store::write(&journal, &lines(&log)).map_err(Fault::Store)?;
        store::set(&env_file, key, value).map_err(Fault::Store)?;
    }

    // The applied marker lands only after every setting is on disk, so a stop
    // before it leaves `applying` over a complete file rather than `applied` over
    // an incomplete one — the next run treats that as a failed apply and offers to
    // resume, which keeps the writes, rather than trusting a half-written stack.
    wizard.transition(Phase::Applied);
    store::write(&progress, &rendered(wizard)).map_err(Fault::Store)
}

/// The data location the operator chose, where they chose one.
fn data_root(wizard: &Wizard) -> Option<PathBuf> {
    wizard.answers().data_location.clone()
}

/// The directories creating `root` will make: every ancestor from the first that
/// is missing down to `root`, parent before child.
///
/// `create_dir_all` makes the whole missing chain, not just the leaf, so all of it
/// is what a reversal must remove. Ordered parent-first here, it is recorded that
/// way and so unwound child-first. `root` is known not to exist when this is
/// called, so the walk always yields at least it.
fn made_by(root: &Path) -> Vec<PathBuf> {
    let mut making = Vec::new();
    let mut here = Some(root);
    while let Some(path) = here.filter(|path| !path.exists()) {
        making.push(path.to_path_buf());
        here = path.parent();
    }
    making.reverse();
    making
}

/// The journal entry for a directory apply created, so it can be removed again.
fn made(path: &Path, stamp: &str) -> Change {
    let path = path.display().to_string();
    Change {
        at: stamp.to_owned(),
        operation: "apply".to_owned(),
        target: path.clone(),
        kind: Kind::Made { path },
    }
}

/// The wizard's progress as the single JSON object the recovery frame reads back.
///
/// A `Progress` cannot fail to serialise, so the empty-string fallback is a shape
/// the type never takes rather than a loss to guard against.
fn rendered(wizard: &Wizard) -> String {
    serde_json::to_string(wizard.progress()).unwrap_or_default()
}

/// The journal as one JSON object per line, the form the recovery frame reads.
///
/// As with the progress, a `Change` cannot fail to serialise, so no line is ever
/// the empty-string fallback.
fn lines(journal: &Journal) -> String {
    journal
        .changes()
        .iter()
        .map(|change| serde_json::to_string(change).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Raised when apply is asked for before the answers have been reviewed.
pub const NOT_REVIEWED: Code = Code::new("SETUP-1");

/// Raised when the operator's chosen data directory cannot be created.
pub const DIR_NOT_MADE: Code = Code::new("SETUP-2");

/// The problem of applying before review — nothing is settled to write.
fn not_reviewed() -> Problem {
    Problem::new(
        NOT_REVIEWED,
        Severity::Error,
        "Setup cannot be applied before it is reviewed",
        "Applying writes the answers to disk, so it runs only once they are all gathered and confirmed. Nothing has been written.",
        Remedy::new("Answer every question, then confirm the review before applying"),
    )
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::apply;
    use crate::config::paths::Paths;
    use crate::config::{store, Protocols};
    use crate::journal::{Change, Kind};
    use crate::platform::Environment;
    use crate::stack::Source;
    use crate::wizard::{Answer, Library, Phase, Wizard};

    /// A journal line for a setting written over nothing — a fresh file, so the
    /// prior value is absent. Built from a key and value stated here, independent
    /// of the plan apply derives, so the test pins what should land rather than
    /// echoing how apply computes it.
    fn fresh_write(key: &str, value: &str) -> Change {
        Change {
            at: "t".to_owned(),
            operation: "apply".to_owned(),
            target: ".env".to_owned(),
            kind: Kind::Set {
                key: key.to_owned(),
                previous: None,
                current: value.to_owned(),
            },
        }
    }

    /// A journal line for a directory apply created, pinned to the path stated
    /// here.
    fn made_dir(path: &Path) -> Change {
        let path = path.display().to_string();
        Change {
            at: "t".to_owned(),
            operation: "apply".to_owned(),
            target: path.clone(),
            kind: Kind::Made { path },
        }
    }

    /// The journal text those changes serialise to, one object per line.
    fn journal_text(changes: &[Change]) -> String {
        changes
            .iter()
            .map(|change| serde_json::to_string(change).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// A scratch directory unique to this process and case, cleared first so a
    /// previous run never leaks into this one.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-apply-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// The install layout under a scratch directory: configuration and data kept
    /// apart, as they are on a real machine.
    fn layout(dir: &Path) -> Paths {
        Paths::rooted(&dir.join("config"), &dir.join("data"))
    }

    /// A stack the operator supplied, already on disk — materialising it writes
    /// nothing, so a test that is not about the stack can ignore it.
    fn external() -> Source {
        Source::External(Path::new("/lemonfiber-not-a-real-stack"))
    }

    /// A wizard on native Linux with every applicable question answered, moved to
    /// review — the state apply expects. The data location is given so the test
    /// controls whether it already exists.
    fn reviewed(data_root: &Path) -> Wizard {
        let mut wizard = Wizard::new(Environment::LinuxNative);
        wizard
            .answer(Answer::Protocols(Protocols::both()))
            .unwrap_or(());
        wizard
            .answer(Answer::DataLocation(data_root.to_path_buf()))
            .unwrap_or(());
        wizard
            .answer(Answer::ServiceUser(Some((1000, 1000))))
            .unwrap_or(());
        wizard
            .answer(Answer::Library(Library::JellyfinDocker))
            .unwrap_or(());
        wizard.answer(Answer::Household(true)).unwrap_or(());
        wizard.answer(Answer::Autostart(false)).unwrap_or(());
        assert!(wizard.transition(Phase::Reviewing), "answers are complete");
        wizard
    }

    #[test]
    fn applying_writes_the_settings_makes_the_data_directory_and_finishes_applied() {
        let dir = scratch("applied");
        let paths = layout(&dir);
        let root = dir.join("data-root");
        let mut wizard = reviewed(&root);

        assert!(apply(&mut wizard, &paths, external(), "t").is_ok());

        // The settings the plan named are on disk, the data directory was created,
        // and the wizard finished applied.
        let file = store::read(&paths.env_file()).unwrap_or_default();
        assert_eq!(file.get("LEMONFIBER_USENET"), Some("on"));
        assert_eq!(
            file.get("DATA_ROOT"),
            Some(root.display().to_string().as_str())
        );
        assert!(root.is_dir(), "the data directory was made");
        assert_eq!(wizard.phase(), Phase::Applied);
    }

    #[test]
    fn the_applied_marker_reaches_disk_so_a_later_run_reads_it() {
        let dir = scratch("marker");
        let paths = layout(&dir);
        let mut wizard = reviewed(&dir.join("data-root"));

        assert!(apply(&mut wizard, &paths, external(), "t").is_ok());

        let saved = std::fs::read_to_string(paths.setup_progress()).unwrap_or_default();
        assert!(saved.contains("\"applied\""), "phase is persisted: {saved}");
    }

    #[test]
    fn every_write_is_journalled_so_it_can_be_unwound() {
        let dir = scratch("journal");
        let paths = layout(&dir);
        let root = dir.join("data-root");
        let mut wizard = reviewed(&root);

        // An external stack writes nothing, so the journal is only the data
        // directory made, then one Set per setting in plan order over a fresh file —
        // pinned to what should land, not to a recomputation of what apply wrote, so
        // a wrong key, a dropped setting, or a missing directory record is caught.
        assert!(apply(&mut wizard, &paths, external(), "t").is_ok());

        let root_shown = root.display().to_string();
        let written = std::fs::read_to_string(paths.journal()).unwrap_or_default();
        assert_eq!(
            written,
            journal_text(&[
                made_dir(&root),
                fresh_write("LEMONFIBER_USENET", "on"),
                fresh_write("LEMONFIBER_TORRENT", "on"),
                fresh_write("DATA_ROOT", &root_shown),
                fresh_write("PUID", "1000"),
                fresh_write("PGID", "1000"),
                fresh_write("JELLYFIN_MODE", "docker"),
            ]),
        );
    }

    #[test]
    fn a_data_directory_several_levels_deep_records_every_directory_it_makes() {
        let dir = scratch("deep");
        let paths = layout(&dir);
        // A parent and its child are both absent, so both must be made and both
        // recorded — parent's line first — or a roll back would strand the parent.
        let parent = dir.join("library-root");
        let root = parent.join("data");
        let mut wizard = reviewed(&root);

        assert!(apply(&mut wizard, &paths, external(), "t").is_ok());

        assert!(root.is_dir(), "the leaf was made");
        let root_shown = root.display().to_string();
        let written = std::fs::read_to_string(paths.journal()).unwrap_or_default();
        assert_eq!(
            written,
            journal_text(&[
                made_dir(&parent),
                made_dir(&root),
                fresh_write("LEMONFIBER_USENET", "on"),
                fresh_write("LEMONFIBER_TORRENT", "on"),
                fresh_write("DATA_ROOT", &root_shown),
                fresh_write("PUID", "1000"),
                fresh_write("PGID", "1000"),
                fresh_write("JELLYFIN_MODE", "docker"),
            ]),
        );
    }

    #[test]
    fn an_existing_data_directory_is_left_alone_and_not_journalled() {
        let dir = scratch("adopt");
        let paths = layout(&dir);
        // The location is already there — the operator's own library to adopt. It
        // must not be recorded as made, so unwinding never removes it.
        let root = dir.join("existing-library");
        assert!(std::fs::create_dir_all(&root).is_ok(), "the library exists");
        let mut wizard = reviewed(&root);

        assert!(apply(&mut wizard, &paths, external(), "t").is_ok());

        let written = std::fs::read_to_string(paths.journal()).unwrap_or_default();
        assert!(
            !written.contains("\"made\""),
            "an existing directory is not recorded as made: {written}",
        );
        assert!(root.is_dir(), "and it is still there");
    }

    #[test]
    fn a_data_directory_that_cannot_be_made_stops_the_apply() {
        let dir = scratch("undir");
        let paths = layout(&dir);
        // A file stands where the data directory's parent would be, so creating the
        // directory under it cannot succeed.
        let blocker = dir.join("a-file");
        assert!(std::fs::create_dir_all(&dir).is_ok());
        assert!(std::fs::write(&blocker, "").is_ok(), "the blocking file");
        let root = blocker.join("data");
        let mut wizard = reviewed(&root);

        let stopped = apply(&mut wizard, &paths, external(), "t");

        assert!(matches!(stopped, Err(problem) if problem.code == super::DIR_NOT_MADE));
        let marker = std::fs::read_to_string(paths.setup_progress()).unwrap_or_default();
        assert!(
            marker.contains("\"applying\""),
            "left mid-apply for recovery"
        );
        // The directory was journalled as made before the attempt that failed, so
        // recovery reads a record for it — removing a path that was not created is
        // a harmless no-op.
        let written = std::fs::read_to_string(paths.journal()).unwrap_or_default();
        assert!(
            written.contains("\"made\""),
            "the attempt was recorded: {written}"
        );
    }

    #[test]
    fn the_embedded_stack_is_written_where_compose_reads_it_but_not_journalled() {
        static EMBEDDED: include_dir::Dir<'_> =
            include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");
        let dir = scratch("stack");
        let paths = layout(&dir);
        let mut wizard = reviewed(&dir.join("data-root"));

        assert!(apply(&mut wizard, &paths, Source::Embedded(&EMBEDDED), "t").is_ok());

        // The stack is on disk where Compose reads it — its manifest among the
        // files. It is lemonfiber's regenerable output, so it is not recorded in the
        // journal: its undo is the next apply rewriting it, not a reversal.
        assert!(
            paths.stack().join("stack.toml").is_file(),
            "the stack was materialised"
        );
        let written = std::fs::read_to_string(paths.journal()).unwrap_or_default();
        let stack_line = journal_text(&[made_dir(&paths.stack())]);
        assert!(
            !written.contains(&stack_line),
            "the stack directory is not journalled: {written}",
        );
    }

    #[test]
    fn an_external_stack_is_left_where_it_is_and_not_written_here() {
        let dir = scratch("external");
        let paths = layout(&dir);
        let mut wizard = reviewed(&dir.join("data-root"));

        assert!(apply(&mut wizard, &paths, external(), "t").is_ok());

        // The operator's own stack stays where it is; nothing is written to the
        // location an embedded stack would land in.
        assert!(!paths.stack().exists(), "no stack was written here");
    }

    #[test]
    fn a_stack_that_cannot_be_written_stops_the_apply() {
        static EMBEDDED: include_dir::Dir<'_> =
            include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");
        let dir = scratch("nostack");
        let paths = layout(&dir);
        // A file sits where the stack's own data directory must be created, so
        // materialising the embedded stack under it cannot succeed.
        assert!(
            std::fs::create_dir_all(dir.join("data")).is_ok(),
            "the data base"
        );
        assert!(
            std::fs::write(paths.data_dir(), "").is_ok(),
            "the blocking file"
        );
        let mut wizard = reviewed(&dir.join("data-root"));

        let stopped = apply(&mut wizard, &paths, Source::Embedded(&EMBEDDED), "t");

        assert!(stopped.is_err(), "the stack could not be written");
        let marker = std::fs::read_to_string(paths.setup_progress()).unwrap_or_default();
        assert!(marker.contains("\"applying\""), "left mid-apply: {marker}");
    }

    #[test]
    fn an_unreviewed_wizard_is_refused_and_writes_nothing() {
        let dir = scratch("unreviewed");
        let paths = layout(&dir);
        // Still gathering answers — apply has nothing settled to write.
        let mut wizard = Wizard::new(Environment::LinuxNative);

        let refused = apply(&mut wizard, &paths, external(), "t");

        assert!(matches!(refused, Err(problem) if problem.code == super::NOT_REVIEWED));
        assert!(!paths.env_file().exists(), "nothing was written");
        assert!(!paths.setup_progress().exists(), "no marker was left");
        assert_eq!(wizard.phase(), Phase::InProgress);
    }

    #[test]
    fn a_stop_partway_through_the_writes_leaves_the_applying_marker_for_recovery() {
        let dir = scratch("interrupted");
        let paths = layout(&dir);
        // Obstruct the journal path with a directory, so the first journal write
        // fails — a stop in the middle of applying, after the applying marker is
        // down but before any setting lands. The recovery-critical property is that
        // the marker reached disk first, so the next run reads a failed apply rather
        // than mistaking a half-done setup for a finished one.
        assert!(
            std::fs::create_dir_all(paths.journal()).is_ok(),
            "obstructing directory"
        );
        let root = dir.join("data-root");
        let mut wizard = reviewed(&root);

        assert!(apply(&mut wizard, &paths, external(), "t").is_err());

        let marker = std::fs::read_to_string(paths.setup_progress()).unwrap_or_default();
        assert!(marker.contains("\"applying\""), "left mid-apply: {marker}");
        assert!(
            !paths.env_file().exists(),
            "no setting was written before the stop"
        );
        assert!(!root.exists(), "the directory was not made before the stop");
    }
}
