use std::path::{Path, PathBuf};

use include_dir::{include_dir, Dir};

use super::{materialise, pending_reverts, reset_stack};
use crate::stack::{Failure, Source};

static STACKLET: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/stacklet");

/// The real disk, which every file here is written to.
const DISK: lemonfiber_adapters::Disk = lemonfiber_adapters::Disk;

/// A clean scratch directory for one test, and the record path beside it.
fn scratch(name: &str) -> (lemonfiber_fixtures::scratch::Scratch, PathBuf) {
    let into = lemonfiber_fixtures::scratch::Scratch::unmade(name).within("stack");
    let record = into.with_file_name("materialised.json");
    (into, record)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn every_file_is_written_and_recorded_then_left_on_a_second_run() {
    let (into, record) = scratch("write-and-leave");
    let source = Source::Embedded(&STACKLET);

    let first = materialise(&DISK, source, Some(&into), Some(&record), &[]);
    let (path, edits) = first.unwrap_or((PathBuf::new(), Vec::new()));
    assert_eq!(path, into.path());
    assert!(edits.is_empty(), "a fresh materialise reports no edits");
    // Every file, including the nested one, is written with the embedded content.
    assert!(read(&into.join("compose.yaml")).contains("image: sonarr"));
    assert!(read(&into.join("stack.toml")).contains("stacklet"));
    assert!(read(&into.join("fragments/tv.yaml")).contains("profiles"));
    assert!(
        read(&record).contains("compose.yaml"),
        "the write is recorded"
    );

    // A second run finds every file exactly as it left it: nothing to report.
    let (_, again) = materialise(&DISK, source, Some(&into), Some(&record), &[])
        .unwrap_or((PathBuf::new(), Vec::new()));
    assert!(again.is_empty(), "an unchanged file is left, not reported");
}

/// A region written into a stack file since is lemonfiber's own, so the next pass
/// neither reports it as the operator's edit nor writes the shipped copy over it.
#[test]
fn a_region_written_since_is_kept_by_the_next_pass_and_not_reported() {
    let (into, record) = scratch("region-kept");
    let source = Source::Embedded(&STACKLET);
    let _ = materialise(&DISK, source, Some(&into), Some(&record), &[]);
    let file = into.join("compose.yaml");
    let with = crate::region::put(&read(&file), "plugin komga", "# komga\n");
    let _ = std::fs::write(&file, &with);
    let mut kept: crate::materialised::Materialised = super::super::record::kept(Some(&record));
    kept.record(
        "compose.yaml",
        crate::materialised::checksum(with.as_bytes()),
    );
    let _ = super::super::record::keep(Some(&record), &kept);
    let with_region = read(&file);

    let edits =
        materialise(&DISK, source, Some(&into), Some(&record), &[]).map(|(_, edits)| edits.len());

    assert_eq!(
        edits.ok(),
        Some(0),
        "nothing is reported as the operator's edit"
    );
    assert_eq!(read(&file), with_region);
}

/// A reset puts back what the operator changed and keeps what lemonfiber wrote,
/// which includes a plugin's region.
#[test]
fn a_reset_puts_back_the_operators_edit_and_keeps_the_region() {
    let (into, record) = scratch("region-reset");
    let source = Source::Embedded(&STACKLET);
    let _ = materialise(&DISK, source, Some(&into), Some(&record), &[]);
    let file = into.join("compose.yaml");
    let shipped = read(&file);
    let with = crate::region::put(&read(&file), "plugin komga", "# komga\n");
    let _ = std::fs::write(&file, &with);
    let mut kept: crate::materialised::Materialised = super::super::record::kept(Some(&record));
    kept.record(
        "compose.yaml",
        crate::materialised::checksum(with.as_bytes()),
    );
    let _ = super::super::record::keep(Some(&record), &kept);
    let _ = std::fs::write(&file, read(&file).replace("sonarr", "my-own-sonarr"));

    let _ = reset_stack(&DISK, source, Some(&into), Some(&record), &[]);

    assert_eq!(
        read(&file),
        crate::region::put(&shipped, "plugin komga", "# komga\n")
    );
}

#[test]
fn an_edited_file_is_preserved_and_reported_with_a_diff() {
    let (into, record) = scratch("preserve-edit");
    let source = Source::Embedded(&STACKLET);
    let _ = materialise(&DISK, source, Some(&into), Some(&record), &[]);

    // The operator edits a materialised file by hand.
    let edited = "services:\n  sonarr:\n    image: my-own-sonarr\n";
    let _ = std::fs::write(into.join("compose.yaml"), edited);

    let (_, edits) = materialise(&DISK, source, Some(&into), Some(&record), &[])
        .unwrap_or((PathBuf::new(), Vec::new()));
    assert_eq!(edits.len(), 1, "only the edited file is reported");
    let edit = edits.first();
    assert!(edit.is_some_and(|edit| edit.path == "compose.yaml"));
    assert!(
        edit.is_some_and(|edit| edit.diff.contains("- ") && edit.diff.contains("+ ")),
        "the diff shows both sides",
    );
    // The operator's edit is left exactly as they made it, not overwritten.
    assert_eq!(read(&into.join("compose.yaml")), edited);
}

#[test]
fn a_reset_reverts_an_edited_file_to_lemonfibers_and_names_it() {
    let (into, record) = scratch("reset-revert");
    let source = Source::Embedded(&STACKLET);
    let (_, _) = materialise(&DISK, source, Some(&into), Some(&record), &[])
        .unwrap_or((PathBuf::new(), Vec::new()));
    let shipped = read(&into.join("compose.yaml"));

    // The operator edits a file, then resets.
    let edited = "services:\n  sonarr:\n    image: my-own-sonarr\n";
    let _ = std::fs::write(into.join("compose.yaml"), edited);

    let (_, reverted) = reset_stack(&DISK, source, Some(&into), Some(&record), &[])
        .unwrap_or((PathBuf::new(), Vec::new()));
    assert_eq!(reverted.len(), 1, "the reverted edit is named");
    assert!(reverted
        .first()
        .is_some_and(|edit| edit.path == "compose.yaml"));
    // The edit is gone: the file is lemonfiber's own again.
    assert_eq!(read(&into.join("compose.yaml")), shipped);
    // And a following materialise sees no drift — the reset re-recorded it.
    let (_, again) = materialise(&DISK, source, Some(&into), Some(&record), &[])
        .unwrap_or((PathBuf::new(), Vec::new()));
    assert!(
        again.is_empty(),
        "the reverted file is no longer read as drift"
    );
}

#[test]
fn a_preview_names_the_reverts_but_writes_nothing() {
    let (into, record) = scratch("reset-preview");
    let source = Source::Embedded(&STACKLET);
    let _ = materialise(&DISK, source, Some(&into), Some(&record), &[]);

    let edited = "services:\n  sonarr:\n    image: my-own-sonarr\n";
    let _ = std::fs::write(into.join("compose.yaml"), edited);

    let pending =
        pending_reverts(&DISK, source, Some(&into), Some(&record), &[]).unwrap_or_default();
    assert_eq!(pending.len(), 1, "the edit that would be reverted is named");
    // The preview touched nothing: the operator's edit is still there.
    assert_eq!(read(&into.join("compose.yaml")), edited);
}

/// A preview of a stack never written names nothing to revert and writes nothing:
/// every file is one lemonfiber would write, and a preview only looks.
#[test]
fn a_preview_of_a_stack_never_written_writes_nothing() {
    let (into, record) = scratch("unwritten-preview");
    let source = Source::Embedded(&STACKLET);

    let pending =
        pending_reverts(&DISK, source, Some(&into), Some(&record), &[]).unwrap_or_default();

    assert!(pending.is_empty(), "{pending:?}");
    assert!(!into.join("compose.yaml").exists(), "nothing was written");
    assert!(!record.exists(), "nothing was recorded");
}

#[test]
fn an_external_stack_is_returned_and_nothing_is_written() {
    let (into, record) = scratch("external");
    let external = Path::new("/some/operator/stack");
    let (path, edits) = materialise(
        &DISK,
        Source::External(external),
        Some(&into),
        Some(&record),
        &[],
    )
    .unwrap_or((PathBuf::new(), vec![]));
    assert_eq!(path, external, "an external stack is used where it lives");
    assert!(edits.is_empty());
    assert!(!into.exists(), "nothing was written for an external stack");
}

#[test]
fn an_embedded_stack_with_nowhere_to_write_is_refused() {
    let refusal = materialise(&DISK, Source::Embedded(&STACKLET), None, None, &[]);
    assert!(matches!(refusal, Err(Failure::NowhereToWrite)));
}

#[test]
fn a_file_where_a_directory_must_go_is_a_write_failure() {
    let (into, record) = scratch("blocked");
    // A plain file sits where the stack directory needs to be, so its parent
    // cannot be made and the write fails rather than guessing.
    if let Some(parent) = into.parent() {
        let _ = std::fs::create_dir_all(parent);
        let _ = std::fs::write(&into, "not a directory");
    }
    let failure = materialise(
        &DISK,
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        &[],
    );
    assert!(matches!(failure, Err(Failure::NotWritten { .. })));
}

#[test]
fn without_a_record_path_the_stack_is_still_written() {
    let (into, _) = scratch("no-record");
    let (_, edits) = materialise(&DISK, Source::Embedded(&STACKLET), Some(&into), None, &[])
        .unwrap_or((PathBuf::new(), vec![]));
    assert!(edits.is_empty());
    assert!(read(&into.join("stack.toml")).contains("stacklet"));
}

/// A declaration is a name and the reason somebody gave for writing it down.
fn declared(area: &str) -> Vec<(String, String)> {
    vec![(
        area.to_owned(),
        "my own edits live in this one and I keep them".to_owned(),
    )]
}

/// A file beneath a declared name is never written — not on a first materialise,
/// and not on the run that would have upgraded it.
#[test]
fn a_file_beneath_a_declared_area_is_never_written() {
    let (into, record) = scratch("unmanaged-file");
    let source = Source::Embedded(&STACKLET);
    let theirs = declared("config/recyclarr");

    let (_, edits) = materialise(&DISK, source, Some(&into), Some(&record), &theirs)
        .unwrap_or((PathBuf::new(), Vec::new()));

    assert!(
        !into.join("config/recyclarr/recyclarr.yml").exists(),
        "a file the operator declared unmanaged was written"
    );
    // And the rest of the stack is written as usual: one declaration is not a
    // refusal to maintain anything else.
    assert!(read(&into.join("compose.yaml")).contains("image: sonarr"));
    // Nor is it reported as an edit held back, which would be reporting drift
    // about an area the operator said is not lemonfiber's to have an opinion on.
    assert!(edits.is_empty(), "{edits:?}");
    // And nothing is recorded for it: a checksum here would read, on the first run
    // after the declaration is taken back, as lemonfiber's own file to overwrite.
    assert!(
        !read(&record).contains("recyclarr"),
        "a file lemonfiber did not write was recorded as though it had"
    );
}

/// Two explicit decisions meet here, and the one that says *never write* wins.
#[test]
fn a_reset_does_not_revert_a_file_the_operator_declared_unmanaged() {
    let (into, record) = scratch("unmanaged-reset");
    let source = Source::Embedded(&STACKLET);
    let theirs = declared("compose.yaml");
    let _ = materialise(&DISK, source, Some(&into), Some(&record), &[]);

    let mine = "services:\n  sonarr:\n    image: an-image-of-my-own\n";
    let _ = std::fs::write(into.join("compose.yaml"), mine);

    let (_, reverted) = reset_stack(&DISK, source, Some(&into), Some(&record), &theirs)
        .unwrap_or((PathBuf::new(), Vec::new()));

    assert_eq!(
        read(&into.join("compose.yaml")),
        mine,
        "a reset wrote over an area the operator had declared unmanaged"
    );
    assert!(reverted.is_empty(), "{reverted:?}");
}

/// A link a container planted where one of its stack files goes is neither read into a
/// diff nor written through: the pass refuses that file by name and the file the link
/// points at keeps what it held.
#[cfg(unix)]
#[test]
fn a_link_planted_where_a_stack_file_goes_is_neither_read_nor_written() {
    let (into, record) = scratch("planted-link");
    let source = Source::Embedded(&STACKLET);
    let operator = into.with_file_name("authorized_keys");
    assert!(std::fs::create_dir_all(into.parent().unwrap_or(&into)).is_ok());
    assert!(std::fs::write(&operator, "ssh-ed25519 theirs\n").is_ok());
    let quality_sync = into.join("config").join("recyclarr");
    let _ = std::fs::create_dir_all(&quality_sync);
    let _ = std::os::unix::fs::symlink(&operator, quality_sync.join("recyclarr.yml"));

    let refused = materialise(&DISK, source, Some(&into), Some(&record), &[]);

    assert!(
        matches!(&refused, Err(Failure::NotWritten { reason, .. }) if reason.contains("is a link")),
        "got: {refused:?}"
    );
    assert_eq!(read(&operator), "ssh-ed25519 theirs\n");
}

mod freshness;
