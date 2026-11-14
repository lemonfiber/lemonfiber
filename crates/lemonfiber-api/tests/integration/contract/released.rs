//! What this build's artefact added, removed or changed against the previous release's.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use lemonfiber_api::contract::layout;
use lemonfiber_api::contract::released::{Diff, Moved, Snapshot, DIFF, RELEASED_DIR, VERSION};
use lemonfiber_fixtures::scratch::Scratch;

use super::fresh;

/// The workspace root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The release the committed artefact was copied from.
fn from() -> String {
    std::fs::read_to_string(root().join(RELEASED_DIR).join(VERSION))
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// What this build's artefact moved against the committed one.
fn diff() -> Diff {
    let before = Snapshot::read(&root().join(RELEASED_DIR));
    assert!(before.is_ok(), "{before:?}");
    Diff::between(
        &from(),
        &before.unwrap_or_default(),
        &Snapshot::of(&fresh()),
    )
}

/// Members given as pairs.
fn members(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone()))
        .collect()
}

#[test]
fn the_committed_diff_is_what_the_two_artefacts_generate() {
    let committed = std::fs::read_to_string(root().join(DIFF)).unwrap_or_default();
    assert_eq!(committed, layout::rendered(&diff()), "run `just contract`");
}

#[test]
fn the_previous_release_is_named_and_its_artefact_read() {
    let from = from();
    assert!(
        from.split('.').count() == 3 && !from.contains('-'),
        "{from}"
    );
    let before = Snapshot::read(&root().join(RELEASED_DIR)).unwrap_or_default();
    assert!(!before.kinds.is_empty());
    assert!(!before.definitions.is_empty());
    assert!(!before.reads.is_empty());
    assert!(!before.refusals.is_empty());
}

#[test]
fn an_artefact_is_unchanged_against_itself_wherever_its_definitions_are_kept() {
    let now = Snapshot::of(&fresh());
    assert!(!now.events.is_empty() && !now.actions.is_empty());
    let alike = Diff::between("0.0.0", &now, &now);
    for part in [
        &alike.kinds,
        &alike.definitions,
        &alike.reads,
        &alike.actions,
        &alike.refusals,
        &alike.events,
    ] {
        assert_eq!(part, &Moved::default());
    }
}

#[test]
fn a_directory_committed_as_the_previous_release_is_read_as_this_build_writes_it() {
    let scratch = Scratch::new("released");
    let dir = scratch.path().join("web-api");
    assert!(layout::replace(&dir, &fresh()).is_ok());
    let read = Snapshot::read(scratch.path());
    assert_eq!(read.ok(), Some(Snapshot::of(&fresh())));
}

#[test]
fn a_single_file_carrying_its_own_definitions_reads_them_as_definitions() {
    let document = json!({
        "kinds": { "job": { "$schema": "x", "properties": { "data": { "$ref": "#/$defs/Started" } }, "$defs": { "Started": { "type": "object" } } } },
        "reads": [{ "path": "/api/version" }],
        "refusals": { "ADMIT-4": { "status": 403 } },
    });
    let read = Snapshot::of_single(&document);
    assert_eq!(
        read.kinds,
        members(&[(
            "job",
            json!({ "properties": { "data": { "$ref": "Started" } } })
        )])
    );
    assert_eq!(
        read.definitions,
        members(&[("Started", json!({ "type": "object" }))])
    );
    assert_eq!(
        read.reads,
        members(&[("/api/version", json!({ "path": "/api/version" }))])
    );
    assert_eq!(read.refusals.len(), 1);
}

#[test]
fn what_was_added_removed_and_changed_is_named_in_each_part() {
    let before = Snapshot {
        kinds: members(&[("job", json!(1)), ("gone", json!(1)), ("same", json!(1))]),
        ..Snapshot::default()
    };
    let now = Snapshot {
        kinds: members(&[("job", json!(2)), ("new", json!(1)), ("same", json!(1))]),
        ..Snapshot::default()
    };
    let diff = Diff::between("0.17.0", &before, &now);
    assert_eq!(
        diff.kinds,
        Moved {
            added: vec!["new".to_owned()],
            removed: vec!["gone".to_owned()],
            changed: vec!["job".to_owned()],
        }
    );
    assert_eq!(diff.from, "0.17.0");
}

#[test]
fn a_previous_release_that_cannot_be_read_is_an_error() {
    let scratch = Scratch::new("released-unread");
    assert!(Snapshot::read(scratch.path()).is_err());
    assert!(std::fs::create_dir_all(scratch.path()).is_ok());
    assert!(std::fs::write(scratch.path().join("web-api.contract.json"), "not json").is_ok());
    assert!(Snapshot::read(scratch.path()).is_err());
}
