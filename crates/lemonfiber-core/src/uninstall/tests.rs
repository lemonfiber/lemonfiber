use super::{naming, reclaimable, Confidence, Foreign, Item, Sort, Tier};

/// A line that is going, occupying the given room.
fn going(name: &str, bytes: u64) -> Item {
    Item {
        name: name.to_owned(),
        sort: Sort::Image,
        what: "an image".to_owned(),
        bytes: Some(bytes),
        kept: None,
        secret: false,
    }
}

/// The same line, kept for a stated reason.
fn kept(name: &str, bytes: u64, why: &str) -> Item {
    Item {
        kept: Some(why.to_owned()),
        ..going(name, bytes)
    }
}

#[test]
fn only_what_is_going_counts_towards_what_would_be_freed() {
    let items = vec![
        going("linuxserver/sonarr:4.0.15", 400),
        kept("postgres:16", 900, "another project is standing on it"),
    ];

    assert_eq!(reclaimable(&items), 400);
    assert!(items.first().is_some_and(Item::goes));
    assert!(items.get(1).is_some_and(|item| !item.goes()));
}

#[test]
fn a_line_with_no_knowable_size_does_not_break_the_total() {
    let items = vec![
        Item {
            bytes: None,
            ..going("sonarr", 0)
        },
        going("radarr", 40),
    ];

    assert_eq!(reclaimable(&items), 40);
}

#[test]
fn a_total_that_would_overflow_saturates_rather_than_wrapping() {
    let items = vec![going("a", u64::MAX), going("b", 1)];

    assert_eq!(reclaimable(&items), u64::MAX);
}

/// The whole of what the agreement is for: the same reading names itself the
/// same way twice, so an answer can be checked against a fresh look.
#[test]
fn the_same_reading_names_itself_the_same_way() {
    let items = vec![going("a", 10)];
    let name = naming(Tier::Media, &items, &[], None);

    assert_eq!(name, naming(Tier::Media, &items, &[], None));
    assert_eq!(name.len(), 8, "{name}");
}

/// A tier changed is a different reading. An operator who read what removing the
/// containers would do has not agreed to losing the library.
#[test]
fn a_different_tier_is_a_different_reading() {
    let items = vec![going("a", 10)];

    assert_ne!(
        naming(Tier::Media, &items, &[], None),
        naming(Tier::Configuration, &items, &[], None)
    );
}

/// A size changed is a different reading, which is the requirement that a
/// confirmation state the volume at stake, held as a property.
#[test]
fn a_disk_that_has_moved_since_it_was_read_is_a_different_reading() {
    assert_ne!(
        naming(Tier::Media, &[going("a", 10)], &[], None),
        naming(Tier::Media, &[going("a", 2_000_000_000_000)], &[], None)
    );
}

/// An image that has since become shared is kept rather than taken, and that is
/// a different reading too — the list the operator agreed to has changed.
#[test]
fn a_line_that_has_become_kept_is_a_different_reading() {
    assert_ne!(
        naming(Tier::Services, &[going("a", 10)], &[], None),
        naming(Tier::Services, &[kept("a", 10, "shared")], &[], None)
    );
}

/// Something appearing beside the library since it was read is a different
/// reading: the operator agreed to a disk that had nothing of theirs on it.
#[test]
fn something_found_beside_the_library_is_a_different_reading() {
    let found = vec![Foreign {
        at: "Photographs".to_owned(),
        files: 9_000,
        bytes: 40,
    }];

    assert_ne!(
        naming(Tier::Media, &[going("a", 10)], &[], None),
        naming(Tier::Media, &[going("a", 10)], &found, None)
    );
}

/// And a data location that turns out to be a network share is a different
/// reading, which is the additional confirmation that case asks for.
#[test]
fn a_removal_that_crosses_a_network_share_is_a_different_reading() {
    assert_ne!(
        naming(Tier::Media, &[going("a", 10)], &[], None),
        naming(Tier::Media, &[going("a", 10)], &[], Some("an SMB share"))
    );
}

#[test]
fn a_reading_that_could_not_be_completed_says_what_it_could_not_read() {
    let whole = Confidence::whole();
    assert!(whole.complete && whole.unread.is_empty());

    let short = whole.short("the container engine is not running");
    assert!(!short.complete);
    assert_eq!(short.unread.len(), 1);
    assert!(short
        .unread
        .first()
        .is_some_and(|why| why.contains("engine")));
}
