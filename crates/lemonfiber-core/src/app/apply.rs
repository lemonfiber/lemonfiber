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
//! Only the environment file is written here — the first, cleanly reversible slice
//! of apply. Creating directories and materialising the stack join it once the
//! journal can record a filesystem change well enough to reverse one.
//!
//! Recovery here is between whole writes, not within one. Each file is written in
//! place, so a stop in the middle of a single write can still tear that one file;
//! making each write atomic — a temporary file renamed over the target — is a
//! hardening the shared writer will grow, and is called out where it bites.

use std::path::Path;

use crate::config::store;
use crate::error::{Code, Diagnose, Problem, Remedy, Severity};
use crate::journal::Journal;
use crate::wizard::{Phase, Wizard};

/// Write a reviewed setup's configuration to disk, driving the lifecycle and
/// recording each write so an interrupted run can be unwound.
///
/// The wizard must be at review — every applicable question answered and
/// confirmed — or there is nothing settled to apply. The settings are written to
/// `env_file`, the lifecycle marker to `progress`, and the record of what was
/// written to `journal`; the marker reaches `applying` on disk before the first
/// write, and each journal entry lands before the write it describes, so a stop at
/// any point leaves a state the next run can recover rather than one it must
/// guess at. The `stamp` times the journal entries, which the wizard has no clock
/// to do itself.
///
/// # Errors
///
/// Returns a [`Problem`] where review has not been reached, or where a file could
/// not be read or written — leaving the marker at `applying` and the journal
/// holding what had been written, which the next run recovers from.
pub fn apply(
    wizard: &mut Wizard,
    env_file: &Path,
    progress: &Path,
    journal: &Path,
    stamp: &str,
) -> Result<(), Box<Problem>> {
    if !wizard.transition(Phase::Applying) {
        return Err(Box::new(not_reviewed()));
    }
    // Every file failure is boxed as one problem on the way out, because a
    // `Problem` is large beside the `()` this returns on success — so the writes
    // themselves stay a plain sequence, each stopping the rest.
    write(wizard, env_file, progress, journal, stamp).map_err(|err| Box::new(err.problem()))
}

/// Perform the writes of an apply, stopping at the first that fails.
///
/// Ordered for recovery: the applying marker is persisted first, each change is
/// journalled before it is written, and the applied marker is persisted last — so
/// a stop at any point leaves the marker and journal a later run reads.
fn write(
    wizard: &mut Wizard,
    env_file: &Path,
    progress: &Path,
    journal: &Path,
    stamp: &str,
) -> Result<(), store::Failure> {
    store::write(progress, &rendered(wizard))?;

    // The whole plan is diffed against the file as it stands before any write, so
    // every recorded `previous` is what was really there. The plan's keys are all
    // distinct, so no write moves a `previous` out from under a later one.
    let before = store::read(env_file)?;
    let plan = wizard.plan();
    let changes = plan.changes(&before, stamp);

    // `changes` is one entry per setting in the same order, so each pairs with the
    // key and value it was built from; they are two views of the same list, walked
    // together.
    let mut log = Journal::new();
    for (change, (key, value)) in changes.into_iter().zip(plan.settings()) {
        // Journalled before it is written: a run that dies between the two leaves a
        // record of a change that may not have landed, and undoing that restores
        // what was already there — harmless. The reverse would leave a real write
        // with nothing to unwind it.
        log.record(change);
        store::write(journal, &lines(&log))?;
        store::set(env_file, key, value)?;
    }

    // The applied marker lands only after every setting is on disk, so a stop
    // before it leaves `applying` over a complete file rather than `applied` over
    // an incomplete one — the next run treats that as a failed apply and offers to
    // resume, which keeps the writes, rather than trusting a half-written stack.
    wizard.transition(Phase::Applied);
    store::write(progress, &rendered(wizard))
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
    use crate::config::{store, Protocols};
    use crate::journal::{Change, Kind};
    use crate::platform::Environment;
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

    /// The three files apply writes, under a scratch config directory.
    fn paths(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
        (
            dir.join(".env"),
            dir.join("setup-progress.json"),
            dir.join("journal.jsonl"),
        )
    }

    /// A wizard on native Linux with every applicable question answered, moved to
    /// review — the state apply expects.
    fn reviewed() -> Wizard {
        let mut wizard = Wizard::new(Environment::LinuxNative);
        wizard
            .answer(Answer::Protocols(Protocols::both()))
            .unwrap_or(());
        wizard
            .answer(Answer::DataLocation(PathBuf::from("/srv/media")))
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
    fn applying_writes_the_settings_and_finishes_applied() {
        let dir = scratch("applied");
        let (env, progress, journal) = paths(&dir);
        let mut wizard = reviewed();

        assert!(apply(&mut wizard, &env, &progress, &journal, "t").is_ok());

        // The settings the plan named are on disk, and the wizard finished applied.
        let file = store::read(&env).unwrap_or_default();
        assert_eq!(file.get("LEMONFIBER_USENET"), Some("on"));
        assert_eq!(file.get("DATA_ROOT"), Some("/srv/media"));
        assert_eq!(wizard.phase(), Phase::Applied);
    }

    #[test]
    fn the_applied_marker_reaches_disk_so_a_later_run_reads_it() {
        let dir = scratch("marker");
        let (env, progress, journal) = paths(&dir);
        let mut wizard = reviewed();

        assert!(apply(&mut wizard, &env, &progress, &journal, "t").is_ok());

        let saved = std::fs::read_to_string(&progress).unwrap_or_default();
        assert!(saved.contains("\"applied\""), "phase is persisted: {saved}");
    }

    #[test]
    fn every_write_is_journalled_so_it_can_be_unwound() {
        let dir = scratch("journal");
        let (env, progress, journal) = paths(&dir);
        let mut wizard = reviewed();

        assert!(apply(&mut wizard, &env, &progress, &journal, "t").is_ok());

        // The journal on disk is one Set per setting, in plan order, each over a
        // fresh file so nothing is there to restore — pinned to keys and values
        // stated here, not to a recomputation of what apply wrote, so a wrong key
        // or a dropped setting would be caught.
        let written = std::fs::read_to_string(&journal).unwrap_or_default();
        assert_eq!(
            written,
            journal_text(&[
                fresh_write("LEMONFIBER_USENET", "on"),
                fresh_write("LEMONFIBER_TORRENT", "on"),
                fresh_write("DATA_ROOT", "/srv/media"),
                fresh_write("PUID", "1000"),
                fresh_write("PGID", "1000"),
                fresh_write("JELLYFIN_MODE", "docker"),
            ]),
        );
    }

    #[test]
    fn an_unreviewed_wizard_is_refused_and_writes_nothing() {
        let dir = scratch("unreviewed");
        let (env, progress, journal) = paths(&dir);
        // Still gathering answers — apply has nothing settled to write.
        let mut wizard = Wizard::new(Environment::LinuxNative);

        let refused = apply(&mut wizard, &env, &progress, &journal, "t");

        assert!(matches!(refused, Err(problem) if problem.code == super::NOT_REVIEWED));
        assert!(!env.exists(), "nothing was written");
        assert!(!progress.exists(), "no marker was left");
        assert_eq!(wizard.phase(), Phase::InProgress);
    }

    #[test]
    fn a_stop_partway_through_the_writes_leaves_the_applying_marker_for_recovery() {
        let dir = scratch("interrupted");
        let (env, progress, _) = paths(&dir);
        // Obstruct the journal path with a directory, so the first change fails to
        // journal — a stop in the middle of applying, after the applying marker is
        // down but before any setting lands. The recovery-critical property is that
        // the marker reached disk first, so the next run reads a failed apply rather
        // than mistaking a half-done setup for a finished one.
        let journal = dir.join("journal-is-a-directory");
        assert!(
            std::fs::create_dir_all(&journal).is_ok(),
            "obstructing directory"
        );
        let mut wizard = reviewed();

        assert!(apply(&mut wizard, &env, &progress, &journal, "t").is_err());

        let marker = std::fs::read_to_string(&progress).unwrap_or_default();
        assert!(marker.contains("\"applying\""), "left mid-apply: {marker}");
        assert!(!env.exists(), "no setting was written before the stop");
    }
}
