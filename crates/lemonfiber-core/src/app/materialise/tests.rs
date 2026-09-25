use std::path::{Path, PathBuf};

use include_dir::{include_dir, Dir};

use super::{materialise, pending_reverts, reapply_recyclarr, recyclarr_customised, reset_stack};
use crate::quality::{Preset, Selection};
use crate::stack::{Failure, Source};

static STACKLET: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/stacklet");

/// A clean scratch directory for one test, and the record path beside it.
fn scratch(name: &str) -> (lemonfiber_fixtures::scratch::Scratch, PathBuf) {
    let into = lemonfiber_fixtures::scratch::Scratch::unmade(name).within("stack");
    let record = into.with_file_name("materialised.json");
    (into, record)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// The default choice, which rewrites the shipped Recyclarr config to itself —
/// the selection the file-writing tests use, since it changes nothing.
fn balanced() -> Selection {
    Selection::everywhere(Preset::Balanced)
}

#[test]
fn every_file_is_written_and_recorded_then_left_on_a_second_run() {
    let (into, record) = scratch("write-and-leave");
    let source = Source::Embedded(&STACKLET);

    let first = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[]);
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
    let (_, again) = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[])
        .unwrap_or((PathBuf::new(), Vec::new()));
    assert!(again.is_empty(), "an unchanged file is left, not reported");
}

/// A region written into a stack file since is lemonfiber's own, so the next pass
/// neither reports it as the operator's edit nor writes the shipped copy over it.
#[test]
fn a_region_written_since_is_kept_by_the_next_pass_and_not_reported() {
    let (into, record) = scratch("region-kept");
    let source = Source::Embedded(&STACKLET);
    let _ = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[]);
    let file = into.join("compose.yaml");
    let _ = super::super::bounded::put(
        &file,
        "compose.yaml",
        "plugin komga",
        "# komga\n",
        Some(&record),
    );
    let with_region = read(&file);

    let edits = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[])
        .map(|(_, edits)| edits.len());

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
    let _ = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[]);
    let file = into.join("compose.yaml");
    let shipped = read(&file);
    let _ = super::super::bounded::put(
        &file,
        "compose.yaml",
        "plugin komga",
        "# komga\n",
        Some(&record),
    );
    let _ = std::fs::write(&file, read(&file).replace("sonarr", "my-own-sonarr"));

    let _ = reset_stack(source, Some(&into), Some(&record), Some(&balanced()), &[]);

    assert_eq!(
        read(&file),
        crate::region::put(&shipped, "plugin komga", "# komga\n")
    );
}

#[test]
fn an_edited_file_is_preserved_and_reported_with_a_diff() {
    let (into, record) = scratch("preserve-edit");
    let source = Source::Embedded(&STACKLET);
    let _ = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[]);

    // The operator edits a materialised file by hand.
    let edited = "services:\n  sonarr:\n    image: my-own-sonarr\n";
    let _ = std::fs::write(into.join("compose.yaml"), edited);

    let (_, edits) = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[])
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
    let (_, _) = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[])
        .unwrap_or((PathBuf::new(), Vec::new()));
    let shipped = read(&into.join("compose.yaml"));

    // The operator edits a file, then resets.
    let edited = "services:\n  sonarr:\n    image: my-own-sonarr\n";
    let _ = std::fs::write(into.join("compose.yaml"), edited);

    let (_, reverted) = reset_stack(source, Some(&into), Some(&record), Some(&balanced()), &[])
        .unwrap_or((PathBuf::new(), Vec::new()));
    assert_eq!(reverted.len(), 1, "the reverted edit is named");
    assert!(reverted
        .first()
        .is_some_and(|edit| edit.path == "compose.yaml"));
    // The edit is gone: the file is lemonfiber's own again.
    assert_eq!(read(&into.join("compose.yaml")), shipped);
    // And a following materialise sees no drift — the reset re-recorded it.
    let (_, again) = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[])
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
    let _ = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[]);

    let edited = "services:\n  sonarr:\n    image: my-own-sonarr\n";
    let _ = std::fs::write(into.join("compose.yaml"), edited);

    let pending = pending_reverts(source, Some(&into), Some(&record), Some(&balanced()), &[])
        .unwrap_or_default();
    assert_eq!(pending.len(), 1, "the edit that would be reverted is named");
    // The preview touched nothing: the operator's edit is still there.
    assert_eq!(read(&into.join("compose.yaml")), edited);
}

#[test]
fn an_external_stack_is_returned_and_nothing_is_written() {
    let (into, record) = scratch("external");
    let external = Path::new("/some/operator/stack");
    let (path, edits) = materialise(
        Source::External(external),
        Some(&into),
        Some(&record),
        Some(&balanced()),
        &[],
    )
    .unwrap_or((PathBuf::new(), vec![]));
    assert_eq!(path, external, "an external stack is used where it lives");
    assert!(edits.is_empty());
    assert!(!into.exists(), "nothing was written for an external stack");
}

#[test]
fn an_embedded_stack_with_nowhere_to_write_is_refused() {
    let refusal = materialise(
        Source::Embedded(&STACKLET),
        None,
        None,
        Some(&balanced()),
        &[],
    );
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
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        Some(&balanced()),
        &[],
    );
    assert!(matches!(failure, Err(Failure::NotWritten { .. })));
}

#[test]
fn without_a_record_path_the_stack_is_still_written() {
    let (into, _) = scratch("no-record");
    let (_, edits) = materialise(
        Source::Embedded(&STACKLET),
        Some(&into),
        None,
        Some(&balanced()),
        &[],
    )
    .unwrap_or((PathBuf::new(), vec![]));
    assert!(edits.is_empty());
    assert!(read(&into.join("stack.toml")).contains("stacklet"));
}

#[test]
fn a_chosen_preset_is_carried_into_the_recyclarr_config() {
    let (into, record) = scratch("recyclarr-maximum");
    let maximum = Selection::everywhere(Preset::Maximum);

    let (_, edits) = materialise(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        Some(&maximum),
        &[],
    )
    .unwrap_or((PathBuf::new(), vec![]));
    // Written, not reported as an edit: this is lemonfiber's own choice landing.
    assert!(edits.is_empty());
    let written = read(&into.join("config/recyclarr/recyclarr.yml"));
    // The 4K includes are in force, and the Balanced ones the fixture shipped
    // with are gone.
    assert!(written.contains("sonarr-web-2160p.yml"));
    assert!(written.contains("radarr-uhd-bluray-web.yml"));
    assert!(!written.contains("sonarr-web-1080p.yml"));
}

#[test]
fn the_default_choice_leaves_the_shipped_recyclarr_config_untouched() {
    let (into, record) = scratch("recyclarr-default");
    let (_, edits) = materialise(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        Some(&balanced()),
        &[],
    )
    .unwrap_or((PathBuf::new(), vec![]));
    assert!(edits.is_empty());
    // The default rewrites the shipped config to itself: byte for byte what the
    // fixture ships, so a stack no one chose a preset for is materialised as before.
    let shipped = include_str!("../../../tests/fixtures/stacklet/config/recyclarr/recyclarr.yml");
    assert_eq!(read(&into.join("config/recyclarr/recyclarr.yml")), shipped);
}

#[test]
fn no_selection_writes_the_rest_but_skips_the_recyclarr_config() {
    // A teardown/restart/rehearsal carries no choice: the stack is still
    // written, but the Recyclarr config is left alone rather than written back
    // to the shipped default.
    let (into, record) = scratch("recyclarr-none");
    let (_, edits) = materialise(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        None,
        &[],
    )
    .unwrap_or((PathBuf::new(), vec![]));
    assert!(edits.is_empty());
    assert!(read(&into.join("compose.yaml")).contains("image: sonarr"));
    assert!(
        !into.join("config/recyclarr/recyclarr.yml").exists(),
        "the Recyclarr config is left untouched, not written",
    );
}

#[test]
fn no_selection_does_not_revert_an_applied_preset() {
    let (into, record) = scratch("recyclarr-no-revert");
    // A preset was applied on a prior up.
    let _ = materialise(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        Some(&Selection::everywhere(Preset::Maximum)),
        &[],
    );
    let recyclarr = into.join("config/recyclarr/recyclarr.yml");
    assert!(read(&recyclarr).contains("sonarr-web-2160p.yml"));

    // A later command carrying no choice leaves the applied preset exactly as it
    // is — not written back to the shipped default.
    let _ = materialise(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        None,
        &[],
    );
    assert!(
        read(&recyclarr).contains("sonarr-web-2160p.yml"),
        "the applied preset is not reverted",
    );
}

#[test]
fn a_config_is_customised_only_once_it_differs_from_the_record() {
    let (into, record) = scratch("customised");
    // Nothing written yet: nothing to be customised against.
    assert!(!recyclarr_customised(Some(&into), Some(&record)));

    // Applied and untouched: lemonfiber's own, not customised.
    let _ = materialise(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        Some(&balanced()),
        &[],
    );
    assert!(!recyclarr_customised(Some(&into), Some(&record)));

    // The operator tunes it by hand: now it is customised.
    let recyclarr = into.join("config/recyclarr/recyclarr.yml");
    let _ = std::fs::write(&recyclarr, "# mine\n");
    assert!(recyclarr_customised(Some(&into), Some(&record)));

    // Deleted while the record persists: nothing on disk to be customised, so a
    // reapply would simply write it again.
    let _ = std::fs::remove_file(&recyclarr);
    assert!(!recyclarr_customised(Some(&into), Some(&record)));
}

#[test]
fn reapply_overwrites_a_customised_config_and_records_it() {
    let (into, record) = scratch("reapply");
    let maximum = Selection::everywhere(Preset::Maximum);
    let _ = materialise(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        Some(&maximum),
        &[],
    );
    let recyclarr = into.join("config/recyclarr/recyclarr.yml");
    // The operator hand-edits it.
    let _ = std::fs::write(&recyclarr, "# mine\n");
    assert!(recyclarr_customised(Some(&into), Some(&record)));

    // Reapply re-asserts the recorded preset over the edit.
    let overwritten = reapply_recyclarr(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        &maximum,
        &[],
        false,
    )
    .unwrap_or_default();
    let edit = overwritten.as_ref();
    assert!(edit.is_some(), "an edit was overwritten");
    assert!(edit.is_some_and(|edit| edit.path.ends_with("recyclarr.yml")));
    // The operator is shown which of their lines went, not only that some did.
    assert!(
        edit.is_some_and(|edit| edit.diff.contains("- # mine")),
        "{overwritten:?}"
    );
    assert!(
        edit.is_some_and(|edit| edit.diff.contains('+')),
        "{overwritten:?}"
    );
    assert!(read(&recyclarr).contains("sonarr-web-2160p.yml"));
    // Recorded as lemonfiber's own again: no longer customised.
    assert!(!recyclarr_customised(Some(&into), Some(&record)));
}

/// The diff of a replaced config reaches a terminal, its scrollback and any bug
/// report pasted out of it, and a Recyclarr config carries a key per instance.
#[test]
fn a_credential_in_the_config_a_reapply_replaces_is_named_and_never_printed() {
    let (into, record) = scratch("reapply-secret");
    let maximum = Selection::everywhere(Preset::Maximum);
    let _ = materialise(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        Some(&maximum),
        &[],
    );
    let recyclarr = into.join("config/recyclarr/recyclarr.yml");
    let key = ["a", "recyclarr", "key"].join("-");
    let _ = std::fs::write(&recyclarr, format!("    api_key: {key}\n"));

    let overwritten = reapply_recyclarr(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        &maximum,
        &[],
        true,
    )
    .unwrap_or_default();
    let shown = overwritten.map(|edit| edit.diff).unwrap_or_default();

    assert!(shown.contains("api_key"), "{shown}");
    assert!(
        !shown.contains(&key),
        "the key survived into the diff: {shown}"
    );
}

#[test]
fn a_reapply_over_a_config_already_in_lemonfibers_own_hand_replaces_nothing() {
    let (into, record) = scratch("reapply-clean");
    let maximum = Selection::everywhere(Preset::Maximum);
    let _ = materialise(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        Some(&maximum),
        &[],
    );

    let overwritten = reapply_recyclarr(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        &maximum,
        &[],
        false,
    )
    .unwrap_or_default();
    assert!(
        overwritten.is_none(),
        "nothing of the operator's was there to lose: {overwritten:?}"
    );
}

#[test]
fn a_rehearsed_reapply_writes_nothing() {
    let (into, record) = scratch("reapply-dry");
    let maximum = Selection::everywhere(Preset::Maximum);
    let _ = materialise(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        Some(&maximum),
        &[],
    );
    let recyclarr = into.join("config/recyclarr/recyclarr.yml");
    let _ = std::fs::write(&recyclarr, "# mine\n");

    let overwritten = reapply_recyclarr(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        &maximum,
        &[],
        true,
    )
    .unwrap_or_default();
    assert!(
        overwritten.is_some(),
        "the rehearsal reports it would overwrite an edit"
    );
    assert!(
        overwritten.is_some_and(|edit| edit.diff.contains("- # mine")),
        "the rehearsal shows what it would replace"
    );
    // The edit is still on disk: a rehearsal changed nothing.
    assert_eq!(read(&recyclarr), "# mine\n");
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

    let (_, edits) = materialise(
        source,
        Some(&into),
        Some(&record),
        Some(&balanced()),
        &theirs,
    )
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
    let _ = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[]);

    let mine = "services:\n  sonarr:\n    image: an-image-of-my-own\n";
    let _ = std::fs::write(into.join("compose.yaml"), mine);

    let (_, reverted) = reset_stack(
        source,
        Some(&into),
        Some(&record),
        Some(&balanced()),
        &theirs,
    )
    .unwrap_or((PathBuf::new(), Vec::new()));

    assert_eq!(
        read(&into.join("compose.yaml")),
        mine,
        "a reset wrote over an area the operator had declared unmanaged"
    );
    assert!(reverted.is_empty(), "{reverted:?}");
}

/// The command whose whole purpose is to overwrite an edit, held by the
/// declaration whose whole purpose is to stop that.
#[test]
fn a_reapply_leaves_a_quality_config_declared_unmanaged_exactly_as_it_is() {
    let (into, record) = scratch("unmanaged-reapply");
    let maximum = Selection::everywhere(Preset::Maximum);
    let _ = materialise(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        Some(&maximum),
        &[],
    );
    let recyclarr = into.join("config/recyclarr/recyclarr.yml");
    let _ = std::fs::write(&recyclarr, "# mine\n");

    let overwritten = reapply_recyclarr(
        Source::Embedded(&STACKLET),
        Some(&into),
        Some(&record),
        &maximum,
        &declared("config/recyclarr/recyclarr.yml"),
        false,
    )
    .unwrap_or_default();

    assert!(overwritten.is_none(), "{overwritten:?}");
    assert_eq!(read(&recyclarr), "# mine\n", "the config was overwritten");
}

#[test]
fn reapply_leaves_an_external_stack_alone() {
    let (into, record) = scratch("reapply-external");
    let overwritten = reapply_recyclarr(
        Source::External(Path::new("/some/operator/stack")),
        Some(&into),
        Some(&record),
        &balanced(),
        &[],
        false,
    )
    .unwrap_or_default();
    assert!(
        overwritten.is_none(),
        "an external stack is the operator's, left untouched"
    );
    assert!(!into.exists(), "nothing was written for an external stack");
}
