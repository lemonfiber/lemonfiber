use std::path::{Path, PathBuf};

use super::{Freshness, Role, Volume};
use crate::ports::filesystem::{FsKind, StorageFacts};
use crate::space::level::Level;

/// The platform's answer for a volume of a given size with a given amount free.
fn facts(point: &str, kind: &str, available: u64, total: u64) -> StorageFacts {
    StorageFacts {
        point: PathBuf::from(point),
        kind: FsKind::classify(kind),
        removable: false,
        available,
        total,
    }
}

#[test]
fn a_volume_reports_what_would_be_left_once_the_queue_has_landed() {
    let measured = Volume::measured(
        Role::Data,
        Path::new("/srv/media"),
        &facts("/srv", "ext4", 100_000, 400_000),
        30_000,
        1_700_000_000,
    );
    assert_eq!(measured.free, Some(100_000));
    assert_eq!(measured.limit, Some(400_000));
    assert_eq!(measured.projected, Some(70_000));
    assert_eq!(measured.at, "/srv/media");
    assert_eq!(measured.point, "/srv");
}

#[test]
fn a_volume_that_could_not_be_read_says_so_rather_than_reading_as_full() {
    let measured = Volume::measured(
        Role::Data,
        Path::new("/srv/media"),
        &facts("", "", 0, 0),
        0,
        1_700_000_000,
    );
    assert_eq!(measured.free, None);
    assert_eq!(measured.limit, None);
    assert_eq!(measured.projected, None);
    assert_eq!(measured.level, Level::Unknown);
}

#[test]
fn a_reading_off_a_share_carries_the_moment_it_was_taken() {
    // Nothing here can make the figure fresher, so what it does is date it.
    let over_the_network = Volume::measured(
        Role::Data,
        Path::new("/mnt/nas/media"),
        &facts("/mnt/nas", "nfs", 100_000, 400_000),
        0,
        1_700_000_000,
    );
    assert_eq!(over_the_network.reading, Freshness::AsOf(1_700_000_000));
    assert!(over_the_network.reading.goes_stale());
}

#[test]
fn a_reading_off_a_local_disk_is_true_as_it_stands() {
    let local = Volume::measured(
        Role::Data,
        Path::new("/srv/media"),
        &facts("/srv", "ext4", 100_000, 400_000),
        0,
        1_700_000_000,
    );
    assert_eq!(local.reading, Freshness::Live);
    assert!(!local.reading.goes_stale());
}

#[test]
fn two_paths_on_one_mount_are_one_volume_and_two_unread_ones_are_not() {
    let data = Volume::measured(
        Role::Data,
        Path::new("/srv/media"),
        &facts("/srv", "ext4", 100_000, 400_000),
        0,
        0,
    );
    let services = Volume::measured(
        Role::Services,
        Path::new("/srv/lemonfiber/config"),
        &facts("/srv", "ext4", 100_000, 400_000),
        0,
        0,
    );
    let elsewhere = Volume::measured(
        Role::Services,
        Path::new("/home/op/.local/share/lemonfiber/config"),
        &facts("/home", "ext4", 900, 4_000),
        0,
        0,
    );
    assert!(data.shares_with(&services));
    assert!(!data.shares_with(&elsewhere));

    let unread = Volume::measured(Role::Data, Path::new("/a"), &facts("", "", 0, 0), 0, 0);
    let also_unread = Volume::measured(Role::Services, Path::new("/b"), &facts("", "", 0, 0), 0, 0);
    assert!(
        !unread.shares_with(&also_unread),
        "two volumes nobody could attribute are not one volume"
    );
}

#[test]
fn each_volume_says_what_it_is_for_and_what_its_filling_costs() {
    for role in [Role::Data, Role::Services] {
        let word = role.word();
        let costs = role.costs();
        assert!(!word.is_empty());
        assert!(costs.len() > 20, "{word} says what it costs: {costs}");
    }
    let worse = Role::Services.costs();
    assert!(
        worse.contains("corrupted"),
        "the one that is worse says why: {worse}"
    );
}
