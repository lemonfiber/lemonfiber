use std::path::PathBuf;

use super::linking;
use crate::ports::filesystem::{FsKind, StorageFacts};

fn facts(point: &str, kind: FsKind) -> StorageFacts {
    StorageFacts {
        point: PathBuf::from(point),
        kind,
        removable: false,
        available: 100_000_000_000,
        total: 500_000_000_000,
    }
}

fn under(point: &str, kind: FsKind) -> (PathBuf, StorageFacts) {
    (PathBuf::from(format!("{point}/data")), facts(point, kind))
}

fn linking_kind() -> FsKind {
    FsKind::Linking("apfs".to_owned())
}

#[test]
fn a_layout_already_under_one_mount_costs_nothing_and_is_not_mentioned() {
    let seen = [under("/srv", linking_kind())];
    assert!(linking(&seen).is_none());
}

#[test]
fn nothing_mounted_says_nothing() {
    assert!(linking(&[]).is_none());
}

#[test]
fn two_filesystems_cannot_hardlink_between_them() {
    let seen = [under("/srv", linking_kind()), under("/mnt", linking_kind())];
    let found = linking(&seen);
    let said = found.as_ref().map(|read| read.because.clone());
    assert_eq!(
        found.as_ref().map(|read| read.links),
        Some(false),
        "{found:?}"
    );
    let said = said.unwrap_or_default();
    assert!(said.contains("/mnt") && said.contains("/srv"), "{said}");
}

#[test]
fn a_filesystem_that_cannot_link_is_named_with_the_reason_it_cannot() {
    let seen = [under("/srv", FsKind::ExFat)];
    let found = linking(&seen);
    assert_eq!(
        found.as_ref().map(|read| read.links),
        Some(false),
        "{found:?}"
    );
    let said = found.map(|read| read.because).unwrap_or_default();
    assert!(said.contains("exFAT"), "{said}");
}

#[test]
fn the_reason_a_filesystem_cannot_outranks_there_being_several() {
    let seen = [under("/srv", FsKind::ExFat), under("/mnt", linking_kind())];
    let said = linking(&seen).map(|read| read.because).unwrap_or_default();
    assert!(said.contains("exFAT"), "{said}");
}

#[test]
fn the_cost_is_quantified_rather_than_described() {
    let seen = [under("/srv", linking_kind()), under("/mnt", linking_kind())];
    let cost = linking(&seen).map(|read| read.cost).unwrap_or_default();
    assert!(cost.contains("free") && cost.contains("copies"), "{cost}");
}

#[test]
fn a_remedy_is_offered_and_never_forced() {
    let seen = [under("/srv", linking_kind()), under("/mnt", linking_kind())];
    let found = linking(&seen);
    assert_eq!(
        found.as_ref().map(|read| read.forced),
        Some(false),
        "the operator decides: {found:?}"
    );
    let remedy = found.map(|read| read.remedy).unwrap_or_default();
    assert!(remedy.contains("one filesystem"), "{remedy}");
}

#[test]
fn the_room_reported_is_the_least_there_is_anywhere() {
    let mut tight = facts("/mnt", linking_kind());
    tight.available = 1;
    let seen = [
        under("/srv", linking_kind()),
        (PathBuf::from("/mnt/x"), tight),
    ];
    let cost = linking(&seen).map(|read| read.cost).unwrap_or_default();
    assert!(cost.contains("1 B"), "{cost}");
}

#[test]
fn two_paths_on_one_mount_are_one_filesystem() {
    let seen = [
        (PathBuf::from("/srv/one"), facts("/srv", linking_kind())),
        (PathBuf::from("/srv/two"), facts("/srv", linking_kind())),
    ];
    assert!(linking(&seen).is_none());
}
