use std::time::Duration;

use super::{Fetching, Importing, Item};

/// An item the client is part-way through.
fn downloading(progress: u8) -> Item {
    Item {
        fetching: Some(Fetching {
            progress,
            moving: true,
        }),
        importing: Some(Importing {
            failures: 0,
            imported: false,
        }),
        ..Item::named("Some.Release")
    }
}

#[test]
fn a_finished_download_nothing_took_is_the_failure_nobody_owns() {
    // To the client it is a completed download; to the *arr it is nothing at
    // all. Neither reports a problem, because neither has one.
    assert!(downloading(100).is_completed_not_imported());
    assert!(!downloading(94).is_completed_not_imported());
}

#[test]
fn a_file_kept_for_seeding_after_it_was_imported_is_the_arrangement_working() {
    // The one case that looks identical from the client's side and is fine.
    let seeding = Item {
        importing: Some(Importing {
            failures: 0,
            imported: true,
        }),
        ..downloading(100)
    };
    assert!(!seeding.is_completed_not_imported());
}

#[test]
fn something_on_disk_that_nothing_is_waiting_for_is_orphaned() {
    let orphan = Item {
        importing: None,
        ..downloading(100)
    };
    assert!(orphan.is_orphaned());
    assert!(!downloading(100).is_orphaned());
}

#[test]
fn something_nothing_has_fetched_is_waiting_rather_than_stalled() {
    // A film that is not out yet has no download to be stalled.
    let wanted = Item {
        fetching: None,
        ..downloading(0)
    };
    assert!(wanted.is_waiting());
    assert!(!downloading(0).is_waiting());
}

#[test]
fn a_fresh_item_has_been_grabbed_once_and_is_managed() {
    // The defaults matter: a caller filling in one field must not accidentally
    // declare something unmanaged or never grabbed.
    let item = Item::named("Some.Release");
    assert_eq!(item.grabs, 1);
    assert!(!item.unmanaged);
    assert_eq!(item.held_for, Duration::ZERO);
}
