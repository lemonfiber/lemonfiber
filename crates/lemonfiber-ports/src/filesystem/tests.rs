use std::path::{Path, PathBuf};

use super::{pick, FsKind, Mount};

/// A mount at a point, with a type and capacity a test does not otherwise
/// care about filled in.
fn mount(point: &str, kind: &str, removable: bool) -> Mount {
    Mount {
        point: PathBuf::from(point),
        kind: kind.to_owned(),
        removable,
        available: 500,
        total: 1_000,
    }
}

#[test]
fn a_network_share_is_recognised_under_either_platforms_name() {
    assert_eq!(FsKind::classify("smbfs"), FsKind::Smb);
    assert_eq!(FsKind::classify("CIFS"), FsKind::Smb);
    assert_eq!(FsKind::classify("nfs4"), FsKind::Nfs);
    assert!(FsKind::classify("smbfs").is_network());
    assert!(FsKind::classify("nfs").is_network());
}

#[test]
fn the_types_that_cannot_link_are_named_specifically() {
    // Each named limitation is stated so the reason reaches the operator: an
    // exFAT volume is told it is exFAT, a share is told it is a share.
    let named = [
        ("exfat", "exFAT"),
        ("vfat", "FAT"),
        ("smbfs", "SMB"),
        ("cifs", "SMB"),
        ("nfs", "NFS"),
        ("9p", "WSL2"),
        ("drvfs", "WSL2"),
    ];
    for (name, mention) in named {
        let stated = FsKind::classify(name).limitation();
        assert!(
            stated.is_some_and(|reason| reason.contains(mention)),
            "{name} should be named as {mention}"
        );
    }
}

#[test]
fn a_filesystem_that_links_carries_no_blame_for_its_type() {
    let ext4 = FsKind::classify("ext4");
    assert_eq!(ext4, FsKind::Linking("ext4".to_owned()));
    assert!(ext4.limitation().is_none());
    assert!(!ext4.is_network());
    assert_eq!(ext4.label(), "ext4");
}

#[test]
fn an_unrecognised_type_is_kept_by_name_rather_than_guessed_at() {
    let odd = FsKind::classify("somenewfs");
    assert_eq!(odd, FsKind::Unknown("somenewfs".to_owned()));
    assert!(odd.limitation().is_none());
    assert_eq!(odd.label(), "somenewfs");
}

#[test]
fn each_named_limitation_reads_as_a_type_a_person_recognises() {
    for name in ["exfat", "fat32", "smbfs", "nfs", "9p"] {
        let kind = FsKind::classify(name);
        assert!(!kind.label().is_empty(), "{name} should have a label");
    }
}

#[test]
fn a_path_is_attributed_to_the_longest_mount_that_contains_it() {
    let mounts = vec![
        mount("/", "apfs", false),
        mount("/Volumes/media", "exfat", true),
    ];
    let facts = pick(&mounts, Path::new("/Volumes/media/tv"));
    assert_eq!(facts.kind, FsKind::ExFat);
    assert!(facts.removable);
    assert_eq!(
        facts.total, 1_000,
        "the chosen mount's capacity comes through"
    );
    assert_eq!(
        facts.point,
        PathBuf::from("/Volumes/media"),
        "the figures name the volume they belong to"
    );
}

#[test]
fn a_limit_imposed_above_the_device_is_the_one_reported() {
    // A dataset with a limit of its own sits inside a far larger device. The
    // total that matters is the one the data will actually hit, and reporting
    // the device's would tell an operator they have four terabytes of room in
    // a place that stops accepting writes at fifty gigabytes.
    let device = Mount {
        point: PathBuf::from("/"),
        kind: "zfs".to_owned(),
        removable: false,
        available: 4_000_000,
        total: 4_000_000,
    };
    let dataset = Mount {
        point: PathBuf::from("/tank/media"),
        kind: "zfs".to_owned(),
        removable: false,
        available: 10_000,
        total: 50_000,
    };
    let facts = pick(&[device, dataset], Path::new("/tank/media/tv/a-show"));
    assert_eq!(facts.total, 50_000, "the effective limit, not the device");
    assert_eq!(facts.available, 10_000);
    assert_eq!(facts.point, PathBuf::from("/tank/media"));
}

#[test]
fn a_path_under_no_reported_mount_has_an_unknown_filesystem() {
    let facts = pick(&[], Path::new("/anywhere"));
    assert_eq!(facts.kind, FsKind::Unknown(String::new()));
    assert!(!facts.removable);
    assert_eq!(facts.total, 0, "nothing to measure where nothing matched");
    assert_eq!(
        facts.point,
        PathBuf::new(),
        "no volume to name where none matched"
    );
}
