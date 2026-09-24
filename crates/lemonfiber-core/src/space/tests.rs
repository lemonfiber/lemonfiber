use std::collections::BTreeSet;
use std::path::PathBuf;

use super::{reckon, Category, Measured, Occupant, Seeded, Stalled, Standing, Volume};
use crate::ports::filesystem::{FsKind, Identity, StorageFacts};
use crate::space::Role;

/// A walked file with a given number of names pointing at it.
fn file(path: &str, bytes: u64, inode: u64, links: u64) -> Occupant {
    Occupant {
        path: PathBuf::from(path),
        bytes,
        identity: Some(Identity { file: inode, links }),
    }
}

/// A volume with room to spare, so a case about the accounting is not also a
/// case about the level.
fn roomy(role: Role, at: &str) -> Volume {
    Volume::measured(
        role,
        &PathBuf::from(at),
        &StorageFacts {
            point: PathBuf::from("/srv"),
            kind: FsKind::classify("ext4"),
            removable: false,
            available: 900_000_000_000,
            total: 1_000_000_000_000,
        },
        0,
        0,
    )
}

/// A stack whose data root holds one imported file and one that was never
/// taken, with the client still holding both.
fn a_stack() -> Measured {
    Measured {
        volumes: vec![roomy(Role::Data, "/srv/media")],
        root: PathBuf::from("/srv/media"),
        data: vec![
            file("/srv/media/downloads/Imported/a.mkv", 8_000, 41, 2),
            file("/srv/media/films/Imported/a.mkv", 8_000, 41, 2),
            file("/srv/media/downloads/Never.Taken/b.mkv", 3_000, 42, 1),
        ],
        services: vec![file("/srv/lemonfiber/config/sonarr.db", 500, 90, 1)],
        landing: 1_000,
        held: vec![
            Seeded {
                name: "Imported".to_owned(),
                bytes: 8_000,
                ratio: 175,
            },
            Seeded {
                name: "Never.Taken".to_owned(),
                bytes: 3_000,
                ratio: 0,
            },
        ],
        awaited: BTreeSet::new(),
        stalled: Vec::new(),
        marked: BTreeSet::new(),
    }
}

/// What one category of the report came to, by heading.
fn line(lines: &[super::Consumption], heading: &str) -> Option<super::Tally> {
    lines
        .iter()
        .find(|line| line.category.heading() == heading)
        .map(|line| line.tally)
}

#[test]
fn a_file_in_two_trees_is_charged_to_the_disk_once() {
    let reckoned = reckon(&a_stack());
    let downloads = line(&reckoned.consumption, "downloads").unwrap_or_default();
    let films = line(&reckoned.consumption, "films").unwrap_or_default();
    assert_eq!(downloads.physical, 11_000, "both downloads, once each");
    assert_eq!(
        films.physical, 0,
        "the library's copy is the same file, already paid for"
    );
    assert_eq!(films.logical, 8_000, "and it is still eight thousand bytes");
    assert!(
        films.differs(),
        "the two readings are worth reporting apart"
    );
}

#[test]
fn every_tree_is_a_line_of_its_own() {
    let reckoned = reckon(&a_stack());
    let headings: Vec<String> = reckoned
        .consumption
        .iter()
        .map(|line| line.category.heading())
        .collect();
    assert!(headings.contains(&"downloads".to_owned()));
    assert!(headings.contains(&"films".to_owned()));
    assert!(headings.contains(&"the services' own files".to_owned()));
    assert!(headings.contains(&"still to land".to_owned()));
}

#[test]
fn what_was_never_imported_is_the_easy_win_and_what_is_seeding_is_not() {
    let reckoned = reckon(&a_stack());
    let orphaned = line(&reckoned.reclaimable, "never imported").unwrap_or_default();
    let seeding = line(&reckoned.reclaimable, "seeding").unwrap_or_default();
    assert_eq!(orphaned.physical, 3_000);
    assert_eq!(seeding.physical, 8_000);

    let offered: Vec<&str> = reckoned
        .candidates
        .iter()
        .filter(|candidate| candidate.offered())
        .map(|candidate| candidate.name.as_str())
        .collect();
    assert_eq!(offered, ["Never.Taken"]);
}

#[test]
fn what_a_confirmed_cleanup_would_take_is_only_what_costs_nothing() {
    let measured = a_stack();
    let taking: Vec<String> = reckon(&measured)
        .offering(&measured)
        .into_iter()
        .map(|occupant| occupant.path.display().to_string())
        .collect();
    assert_eq!(taking, ["/srv/media/downloads/Never.Taken/b.mkv"]);
}

#[test]
fn nothing_is_offered_for_what_the_operator_asked_to_be_left_alone() {
    let mut measured = a_stack();
    measured.marked = BTreeSet::from(["Never.Taken".to_owned()]);
    let reckoned = reckon(&measured);
    assert!(
        reckoned.offering(&measured).is_empty(),
        "the instruction is followed rather than weighed"
    );
    assert!(reckoned
        .candidates
        .iter()
        .any(|candidate| candidate.standing == Standing::LeftAlone));
    assert!(
        line(&reckoned.reclaimable, "left alone at your request").is_some(),
        "and the room it takes is still accounted for"
    );
}

#[test]
fn an_offer_that_has_moved_on_names_itself_differently() {
    let first = reckon(&a_stack());
    let mut later = a_stack();
    later.held.push(Seeded {
        name: "Also.Never.Taken".to_owned(),
        bytes: 5_000,
        ratio: 0,
    });
    later.data.push(file(
        "/srv/media/downloads/Also.Never.Taken/c.mkv",
        5_000,
        43,
        1,
    ));
    assert_ne!(first.agreement, reckon(&later).agreement);
    assert_eq!(first.agreement.len(), 8, "{}", first.agreement);
}

#[test]
fn the_stack_stands_where_its_worst_volume_stands_and_says_when_it_halts() {
    let mut measured = a_stack();
    measured.volumes.push(Volume::measured(
        Role::Services,
        &PathBuf::from("/home/op/.local/share/lemonfiber"),
        &StorageFacts {
            point: PathBuf::from("/home"),
            kind: FsKind::classify("ext4"),
            removable: false,
            available: 1_000,
            total: 1_000_000_000,
        },
        0,
        0,
    ));
    let reckoned = reckon(&measured);
    assert_eq!(reckoned.level, crate::space::Level::Exhausted);
    assert!(reckoned.halted);
    assert_eq!(reckoned.volumes.len(), 2, "both are reported");
}

#[test]
fn space_freed_outside_this_product_clears_the_condition_on_the_next_reading() {
    // Nothing is kept between runs, so a disk somebody emptied by hand reads as
    // emptied rather than as whatever the last verdict was.
    let mut full = a_stack();
    full.volumes = vec![Volume::measured(
        Role::Data,
        &PathBuf::from("/srv/media"),
        &StorageFacts {
            point: PathBuf::from("/srv"),
            kind: FsKind::classify("ext4"),
            removable: false,
            available: 1_000,
            total: 1_000_000_000,
        },
        0,
        0,
    )];
    assert!(reckon(&full).halted);
    assert!(!reckon(&a_stack()).halted, "the same code, a fuller disk");
}

#[test]
fn an_import_that_stopped_part_way_is_named_with_what_is_on_disk_for_it() {
    let mut measured = a_stack();
    measured.stalled = vec![
        Stalled {
            name: "Never.Taken".to_owned(),
            said: Some("No space left on device".to_owned()),
        },
        Stalled {
            name: "Unheard.Of".to_owned(),
            said: None,
        },
    ];
    let reckoned = reckon(&measured);
    assert_eq!(reckoned.interrupted.len(), 2);
    let named = reckoned.interrupted.first();
    assert!(
        named.is_some_and(
            |stopped| stopped.partial == 3_000 && stopped.said == "No space left on device"
        ),
        "{named:?}"
    );
    let silent = reckoned.interrupted.get(1);
    assert!(
        silent.is_some_and(|stopped| stopped.partial == 0 && stopped.said.contains("no reason")),
        "silence is said as silence: {silent:?}"
    );
}

#[test]
fn a_file_sitting_in_the_root_itself_is_still_accounted_for() {
    let mut measured = a_stack();
    measured.data.push(file("/srv/media/loose.mkv", 700, 99, 1));
    let reckoned = reckon(&measured);
    assert_eq!(
        line(&reckoned.consumption, "the data location itself")
            .unwrap_or_default()
            .physical,
        700
    );
}

#[test]
fn a_walked_file_outside_the_root_is_named_by_what_it_is_under() {
    // Nothing should produce one, and a walk that did must not lose it.
    let mut measured = a_stack();
    measured.data = vec![file("/elsewhere/odd/one.mkv", 5, 51, 1)];
    let reckoned = reckon(&measured);
    assert_eq!(
        line(&reckoned.consumption, "elsewhere")
            .unwrap_or_default()
            .physical,
        5
    );
}

#[test]
fn a_stack_with_nothing_on_it_reports_no_empty_lines() {
    let reckoned = reckon(&Measured::default());
    assert!(reckoned.consumption.is_empty());
    assert!(reckoned.reclaimable.is_empty());
    assert!(reckoned.candidates.is_empty());
    assert!(reckoned.outsized.is_empty());
    assert!(reckoned.reclaimed.is_none());
    assert_eq!(reckoned.level, crate::space::Level::Unknown);
    assert!(!reckoned.halted);
}

#[test]
fn archive_parts_beside_what_was_unpacked_from_them_are_offered() {
    let mut measured = a_stack();
    measured
        .data
        .push(file("/srv/media/downloads/Done/a.rar", 400, 61, 1));
    measured
        .data
        .push(file("/srv/media/downloads/Done/Done.mkv", 900, 62, 1));
    let reckoned = reckon(&measured);
    assert_eq!(
        line(&reckoned.reclaimable, "archives already unpacked")
            .unwrap_or_default()
            .physical,
        400
    );
    let taking: Vec<String> = reckoned
        .offering(&measured)
        .into_iter()
        .map(|occupant| occupant.path.display().to_string())
        .collect();
    assert_eq!(
        taking,
        [
            "/srv/media/downloads/Done/a.rar",
            "/srv/media/downloads/Never.Taken/b.mkv"
        ]
    );
}

#[test]
fn nothing_offered_twice_where_a_download_is_also_an_unpacked_archive() {
    let mut measured = a_stack();
    measured
        .data
        .push(file("/srv/media/downloads/Never.Taken/c.rar", 400, 63, 1));
    let taking: Vec<String> = reckon(&measured)
        .offering(&measured)
        .into_iter()
        .map(|occupant| occupant.path.display().to_string())
        .collect();
    assert_eq!(
        taking,
        [
            "/srv/media/downloads/Never.Taken/b.mkv",
            "/srv/media/downloads/Never.Taken/c.rar"
        ],
        "each path once, however many reasons there are to take it"
    );
}

#[test]
fn every_reclaim_code_this_module_raises_belongs_to_it() {
    for code in [
        super::HALTED,
        super::NOWHERE_TO_MEASURE,
        super::WALK_REFUSED,
        super::NOTHING_TO_ASK,
        super::NOT_HELD,
        super::ANOTHER_OFFER,
        super::STILL_HELD,
    ] {
        // Bound rather than called inside the message: an argument to a
        // passing assertion is never evaluated, and a line nothing evaluates
        // is one the coverage gate counts against a file that looks tested.
        let named = code.as_str();
        assert!(named.starts_with("SPACE-"), "{named} is this feature's own");
    }
}

#[test]
fn the_category_of_a_line_decides_what_reclaiming_it_costs() {
    let reckoned = reckon(&a_stack());
    for line in reckoned.consumption.iter().chain(&reckoned.reclaimable) {
        assert_eq!(line.reclaim, line.category.reclaim());
    }
    assert!(
        reckoned
            .consumption
            .iter()
            .any(|line| matches!(line.category, Category::Tree(_))),
        "there was a tree to check the rule against"
    );
}
