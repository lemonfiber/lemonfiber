use crate::test_support::a_fresh_write;
use std::path::Path;

use lemonfiber_fixtures::ports::Chance;

use super::{apply, Applying, Baseline, SETTINGS};
use crate::alert::{Appetite, Wants};
use crate::autostart::Wanted;
use crate::config::paths::Paths;
use crate::config::{store, Protocols};
use crate::journal::{Change, Kind};
use crate::platform::Environment;
use crate::stack::Source;
use crate::wizard::{Answer, Library, Phase, Vpn, Wizard};

/// What an apply writes with, over a real directory and a real machine's
/// randomness — the key the journal's credentials are sealed under is made from
/// the latter.
fn applying<'a>(paths: &'a Paths, source: Source, stamp: &'a str) -> Applying<'a> {
    Applying {
        paths,
        source,
        stamp,
        random: &A_MACHINE,
    }
}

/// The randomness a real machine supplies.
static A_MACHINE: Chance = Chance::cycling();

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
fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::unmade(name)
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
///
/// Autostart is declined, which is the terminal's own default. A test about
/// *that* answer takes [`answering_autostart`] instead, so the two cannot
/// agree by accident.
fn reviewed(data_root: &Path) -> Wizard {
    answering_autostart(data_root, false)
}

/// The same wizard, answering the autostart question either way.
///
/// Split out rather than parameterising every call site: what apply does with
/// the answer is one test's subject and every other test's background noise.
fn answering_autostart(data_root: &Path, on_boot: bool) -> Wizard {
    let mut wizard = Wizard::new(Environment::LinuxNative);
    wizard
        .answer(Answer::Protocols(Protocols::both()))
        .unwrap_or(());
    wizard.answer(Answer::Vpn(Vpn::Carrying)).unwrap_or(());
    wizard
        .answer(Answer::DataLocation(data_root.to_path_buf()))
        .unwrap_or(());
    wizard.answer(Answer::Credentials(None)).unwrap_or(());
    wizard.answer(Answer::Provider(None)).unwrap_or(());
    wizard
        .answer(Answer::ServiceUser(Some((1000, 1000))))
        .unwrap_or(());
    wizard
        .answer(Answer::Library(Library::JellyfinDocker))
        .unwrap_or(());
    wizard.answer(Answer::Household(true)).unwrap_or(());
    wizard
        .answer(Answer::Notifications(Appetite::default_appetite()))
        .unwrap_or(());
    wizard.answer(Answer::Autostart(on_boot)).unwrap_or(());
    assert!(wizard.transition(Phase::Reviewing), "answers are complete");
    wizard
}

#[test]
fn applying_writes_the_settings_makes_the_data_directory_and_finishes_applied() {
    let dir = scratch("applied");
    let paths = layout(&dir);
    let root = dir.join("data-root");
    let mut wizard = reviewed(&root);

    assert!(apply(&mut wizard, &applying(&paths, external(), "t")).is_ok());

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

/// The record beside the settings, as a later change reads it back.
fn remembering(paths: &Paths) -> Baseline {
    let text = std::fs::read_to_string(paths.baseline()).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

#[test]
fn what_setup_wrote_is_remembered_so_a_later_hand_edit_can_be_told_from_it() {
    // Without this, the first change made to any setting has no third value to
    // judge against: an edit made outside lemonfiber and the value setup itself
    // wrote look identical, and the change would take the edit with it.
    let dir = scratch("remembered");
    let paths = layout(&dir);
    let mut wizard = reviewed(&dir.join("data-root"));

    assert!(apply(&mut wizard, &applying(&paths, external(), "t")).is_ok());

    assert_eq!(
        remembering(&paths)
            .entry(SETTINGS, "LEMONFIBER_USENET")
            .map(|record| record.value.clone()),
        Some("on".to_owned())
    );
}

#[test]
fn a_record_beside_the_settings_keeps_what_seeding_put_there() {
    // An apply beside an already-seeded stack must add to the record rather than
    // write over it: the services' own values are what a later seed compares
    // against, and losing them makes every one of them read as unmanaged.
    let dir = scratch("merged");
    let paths = layout(&dir);
    let mut seeded = Baseline::new();
    seeded.record("sonarr", "rootfolder", "/data/media/tv", "t");
    let _ = store::write(
        &paths.baseline(),
        &serde_json::to_string(&seeded).unwrap_or_default(),
    );
    let mut wizard = reviewed(&dir.join("data-root"));

    assert!(apply(&mut wizard, &applying(&paths, external(), "t")).is_ok());

    let held = remembering(&paths);
    assert!(held.entry("sonarr", "rootfolder").is_some());
    assert!(held.entry(SETTINGS, "DATA_ROOT").is_some());
}

#[test]
fn a_record_that_cannot_be_opened_at_all_stops_nothing_and_is_left_alone() {
    // A directory where the record should be: not there in the sense a first run
    // means, and not something to write over either. The apply is what matters
    // here and it still finishes; the record is best-effort by design.
    let dir = scratch("blocked");
    let paths = layout(&dir);
    let _ = std::fs::create_dir_all(paths.baseline());
    let mut wizard = reviewed(&dir.join("data-root"));

    assert!(apply(&mut wizard, &applying(&paths, external(), "t")).is_ok());

    assert!(paths.baseline().is_dir(), "left exactly as it was");
}

#[test]
fn a_record_that_cannot_be_read_is_left_where_it_is() {
    // The same line seeding holds: a record that may be there but unreadable is
    // safer left for the operator to re-form than silently replaced.
    let dir = scratch("unreadable");
    let paths = layout(&dir);
    let _ = store::write(&paths.baseline(), "not json at all");
    let mut wizard = reviewed(&dir.join("data-root"));

    assert!(apply(&mut wizard, &applying(&paths, external(), "t")).is_ok());

    let text = std::fs::read_to_string(paths.baseline()).unwrap_or_default();
    assert_eq!(text, "not json at all");
}

#[test]
fn the_applied_marker_reaches_disk_so_a_later_run_reads_it() {
    let dir = scratch("marker");
    let paths = layout(&dir);
    let mut wizard = reviewed(&dir.join("data-root"));

    assert!(apply(&mut wizard, &applying(&paths, external(), "t")).is_ok());

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
    assert!(apply(&mut wizard, &applying(&paths, external(), "t")).is_ok());

    let root_shown = root.display().to_string();
    let written = std::fs::read_to_string(paths.journal()).unwrap_or_default();
    assert_eq!(
        written,
        journal_text(&[
            made_dir(&root),
            a_fresh_write("LEMONFIBER_USENET", "on"),
            a_fresh_write("LEMONFIBER_TORRENT", "on"),
            a_fresh_write("DATA_ROOT", &root_shown),
            a_fresh_write("PUID", "1000"),
            a_fresh_write("PGID", "1000"),
            a_fresh_write("JELLYFIN_MODE", "docker"),
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

    assert!(apply(&mut wizard, &applying(&paths, external(), "t")).is_ok());

    assert!(root.is_dir(), "the leaf was made");
    let root_shown = root.display().to_string();
    let written = std::fs::read_to_string(paths.journal()).unwrap_or_default();
    assert_eq!(
        written,
        journal_text(&[
            made_dir(&parent),
            made_dir(&root),
            a_fresh_write("LEMONFIBER_USENET", "on"),
            a_fresh_write("LEMONFIBER_TORRENT", "on"),
            a_fresh_write("DATA_ROOT", &root_shown),
            a_fresh_write("PUID", "1000"),
            a_fresh_write("PGID", "1000"),
            a_fresh_write("JELLYFIN_MODE", "docker"),
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

    assert!(apply(&mut wizard, &applying(&paths, external(), "t")).is_ok());

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

    let stopped = apply(&mut wizard, &applying(&paths, external(), "t"));

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

    assert!(apply(
        &mut wizard,
        &applying(&paths, Source::Embedded(&EMBEDDED), "t")
    )
    .is_ok());

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

    assert!(apply(&mut wizard, &applying(&paths, external(), "t")).is_ok());

    // The operator's own stack stays where it is; nothing is written to the
    // location an embedded stack would land in.
    assert!(!paths.stack().exists(), "no stack was written here");
}

/// The case that made routing this through the ordinary comparison worth doing.
///
/// Setup is reached more than once on a machine that has been set up: a resumed
/// apply, a recovered one, and setup asked for again all land here, and the
/// extraction this replaced would have written over whatever was on disk.
#[test]
fn a_second_apply_leaves_a_stack_file_the_operator_edited_exactly_as_they_set_it() {
    static EMBEDDED: include_dir::Dir<'_> =
        include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");
    let dir = scratch("stack-edited");
    let paths = layout(&dir);
    let mut first = reviewed(&dir.join("data-root"));
    assert!(apply(
        &mut first,
        &applying(&paths, Source::Embedded(&EMBEDDED), "t")
    )
    .is_ok());

    let compose = paths.stack().join("compose.yml");
    let theirs = "services:\n  sonarr:\n    image: an-image-of-my-own\n";
    assert!(
        std::fs::write(&compose, theirs).is_ok(),
        "the operator's edit"
    );

    let mut again = reviewed(&dir.join("data-root"));
    assert!(apply(
        &mut again,
        &applying(&paths, Source::Embedded(&EMBEDDED), "t")
    )
    .is_ok());

    assert_eq!(
        std::fs::read_to_string(&compose).unwrap_or_default(),
        theirs,
        "a second apply wrote over a file the operator had changed by hand"
    );
    // And the rest of the stack is still lemonfiber's own: preserving one file is
    // not declining to write the others.
    assert!(paths.stack().join("stack.toml").is_file());
}

/// What makes the comparison above possible: a record of what was written, kept
/// where the next run reads it. Without it, the first change to any of these files
/// reads as an operator's edit — the safe direction, and a stack that never
/// upgrades.
#[test]
fn an_apply_records_the_checksums_of_what_it_wrote() {
    static EMBEDDED: include_dir::Dir<'_> =
        include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");
    let dir = scratch("stack-recorded");
    let paths = layout(&dir);
    let mut wizard = reviewed(&dir.join("data-root"));
    assert!(apply(
        &mut wizard,
        &applying(&paths, Source::Embedded(&EMBEDDED), "t")
    )
    .is_ok());

    let recorded = std::fs::read_to_string(paths.materialised()).unwrap_or_default();
    assert!(recorded.contains("compose.yml"), "{recorded}");
    assert!(recorded.contains("stack.toml"), "{recorded}");
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

    let stopped = apply(
        &mut wizard,
        &applying(&paths, Source::Embedded(&EMBEDDED), "t"),
    );

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

    let refused = apply(&mut wizard, &applying(&paths, external(), "t"));

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

    assert!(apply(&mut wizard, &applying(&paths, external(), "t")).is_err());

    let marker = std::fs::read_to_string(paths.setup_progress()).unwrap_or_default();
    assert!(marker.contains("\"applying\""), "left mid-apply: {marker}");
    assert!(
        !paths.env_file().exists(),
        "no setting was written before the stop"
    );
    assert!(!root.exists(), "the directory was not made before the stop");
}

#[test]
fn the_notification_answer_is_written_where_a_later_run_reads_it() {
    // Gathered at setup and then discarded would be worse than never asking:
    // the operator would believe they had chosen something.
    let dir = scratch("appetite");
    let paths = layout(&dir);
    let root = dir.join("data-root");
    let mut wizard = reviewed(&root);
    assert!(apply(&mut wizard, &applying(&paths, external(), "1000")).is_ok());

    let written = std::fs::read_to_string(paths.notifications()).unwrap_or_default();
    assert_eq!(
        serde_json::from_str::<Wants>(&written).ok(),
        Some(Wants::preset(Appetite::default_appetite())),
        "{written}"
    );
}

/// The autostart answer as a later run reads it back off disk.
fn kept_autostart(paths: &Paths) -> Option<Wanted> {
    let written = std::fs::read_to_string(paths.autostart()).unwrap_or_default();
    serde_json::from_str(&written).ok()
}

#[test]
fn the_autostart_answer_is_written_where_a_later_run_reads_it() {
    // Asked with its cost attached and then thrown away is the worst of both:
    // the operator has weighed a consequence and been told nothing came of it.
    // Both answers are checked, because a record that always says the same
    // thing would pass a test of one of them.
    for asked in [true, false] {
        let dir = scratch(if asked { "boot-yes" } else { "boot-no" });
        let paths = layout(&dir);
        let mut wizard = answering_autostart(&dir.join("data-root"), asked);

        assert!(apply(&mut wizard, &applying(&paths, external(), "1000")).is_ok());

        assert_eq!(kept_autostart(&paths), Some(Wanted::answered(asked)));
    }
}

#[test]
fn an_autostart_answer_that_cannot_be_written_stops_the_apply() {
    // The last thing written before the applied marker, so failing here must
    // leave `applying` rather than a stack recorded as fully set up carrying an
    // answer nobody kept. The same standard the appetite is held to, and for a
    // stronger reason: this one is about whether the stack exists after a
    // reboot.
    let dir = scratch("no-autostart");
    let paths = layout(&dir);
    assert!(
        std::fs::create_dir_all(paths.autostart()).is_ok(),
        "a directory sits where the answer's file must go"
    );
    let mut wizard = reviewed(&dir.join("data-root"));

    let stopped = apply(&mut wizard, &applying(&paths, external(), "t"));

    assert!(stopped.is_err());
    assert_ne!(wizard.phase(), crate::wizard::Phase::Applied);
}

#[test]
fn a_notification_answer_that_cannot_be_written_stops_the_apply() {
    // It is the last thing written before the applied marker, so failing here
    // must leave `applying` rather than a stack recorded as fully set up with
    // an answer nobody kept.
    let dir = scratch("no-appetite");
    let paths = layout(&dir);
    assert!(
        std::fs::create_dir_all(paths.notifications()).is_ok(),
        "a directory sits where the answer's file must go"
    );
    let mut wizard = reviewed(&dir.join("data-root"));

    let stopped = apply(&mut wizard, &applying(&paths, external(), "t"));

    assert!(stopped.is_err());
    assert_ne!(wizard.phase(), crate::wizard::Phase::Applied);
}

/// An apply over a machine with a record of earlier changes keeps that record: a
/// resumed or repeated setup that started an empty journal would write the history of
/// everything before it over.
#[test]
fn an_apply_keeps_what_the_journal_already_held() {
    let dir = scratch("keeps-journal");
    let paths = layout(&dir);
    let earlier = crate::app::recover::journalled(
        &paths.journal(),
        &[crate::test_support::a_fresh_write("EARLIER", "kept")],
        &lemonfiber_fixtures::ports::Chance::cycling(),
    );
    assert!(earlier.is_ok(), "{earlier:?}");
    let mut wizard = reviewed(&dir.join("library"));

    assert!(apply(&mut wizard, &applying(&paths, external(), "t")).is_ok());

    let held = crate::app::recover::journal_at(&paths.journal()).unwrap_or_default();
    assert!(
        held.changes().iter().any(|change| matches!(
            &change.kind,
            crate::journal::Kind::Set { key, .. } if key == "EARLIER"
        )),
        "the earlier change is still recorded: {:?}",
        held.changes()
    );
}

/// A recovery over a journal that cannot be read is refused, and what setup had written
/// is said to be unreadable rather than listed as nothing.
#[test]
fn a_recovery_over_a_journal_that_cannot_be_read_is_refused() {
    let dir = scratch("recover-unreadable");
    let paths = layout(&dir);
    assert!(std::fs::create_dir_all(paths.journal().join("held")).is_ok());
    let mut wizard = reviewed(&dir.join("library"));

    let refused = crate::app::setup::recovered(
        &mut wizard,
        &applying(&paths, external(), "t"),
        crate::wizard::Choice::RollBack,
    );
    assert!(
        refused.is_err_and(|problem| problem.summary.contains("could not be read")),
        "a recovery read a journal it could not open as one with nothing to put back"
    );

    let said = crate::app::setup::written_so_far(&paths);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said.first()
            .is_some_and(|line| line.contains("could not be read")),
        "{said:?}"
    );
}
