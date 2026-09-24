use std::path::PathBuf;

use super::{tally, Counting};
use crate::ports::filesystem::Identity;
use crate::ports::occupancy::Occupant;

/// A file of a given size, under a given inode with a given number of names.
fn file(path: &str, bytes: u64, inode: u64, links: u64) -> Occupant {
    Occupant {
        path: PathBuf::from(path),
        bytes,
        identity: Some(Identity { file: inode, links }),
    }
}

/// A file the platform would not identify.
fn nameless(path: &str, bytes: u64) -> Occupant {
    Occupant {
        path: PathBuf::from(path),
        bytes,
        identity: None,
    }
}

#[test]
fn a_file_reachable_under_two_names_is_paid_for_once() {
    let counted = tally(&[
        file("/data/downloads/a.mkv", 8_000, 41, 2),
        file("/data/media/a.mkv", 8_000, 41, 2),
    ]);
    assert_eq!(counted.logical, 16_000, "what the names add up to");
    assert_eq!(counted.physical, 8_000, "what the disk actually lost");
    assert_eq!(counted.files, 2);
    assert_eq!(counted.shared, 1);
    assert!(counted.differs());
    assert_eq!(counted.saved(), 8_000);
}

#[test]
fn nothing_shared_reads_as_one_figure_rather_than_two() {
    let counted = tally(&[
        file("/data/media/a.mkv", 8_000, 41, 1),
        file("/data/media/b.mkv", 2_000, 42, 1),
    ]);
    assert_eq!(counted.logical, 10_000);
    assert_eq!(counted.physical, 10_000);
    assert_eq!(counted.shared, 0);
    assert!(!counted.differs(), "one figure, not two of the same");
    assert_eq!(counted.saved(), 0);
}

#[test]
fn a_file_the_platform_would_not_identify_is_charged_in_full() {
    // Two unidentified files of the same size are not evidence of one file, and
    // a total short by a real file is worse than one long by an unread one.
    let counted = tally(&[
        nameless("/data/a.mkv", 5_000),
        nameless("/data/b.mkv", 5_000),
    ]);
    assert_eq!(counted.physical, 10_000);
    assert_eq!(counted.shared, 0);
}

#[test]
fn a_zero_identity_is_no_identity() {
    let counted = tally(&[
        file("/data/a.mkv", 5_000, 0, 1),
        file("/data/b.mkv", 5_000, 0, 1),
    ]);
    assert_eq!(
        counted.physical, 10_000,
        "two zeroes are not evidence of one file"
    );
    assert_eq!(counted.shared, 0);
}

#[test]
fn a_file_shared_between_two_trees_is_charged_to_the_first_of_them() {
    // The reason the count is held across trees rather than restarted per tree:
    // each tree counted alone is right about itself, and the sum of them is
    // twice the truth.
    let mut counting = Counting::default();
    let downloads = counting.count(&[file("/data/downloads/a.mkv", 8_000, 41, 2)]);
    let library = counting.count(&[file("/data/media/a.mkv", 8_000, 41, 2)]);
    assert_eq!(downloads.physical, 8_000);
    assert_eq!(library.physical, 0, "the second tree pays nothing again");
    assert_eq!(library.shared, 1);
    assert_eq!(
        downloads.physical + library.physical,
        8_000,
        "the volume lost eight thousand bytes, not sixteen"
    );
}

#[test]
fn counting_nothing_comes_to_nothing() {
    let counted = tally(&[]);
    assert_eq!(counted.logical, 0);
    assert_eq!(counted.physical, 0);
    assert_eq!(counted.files, 0);
}

#[test]
fn a_size_large_enough_to_wrap_saturates_instead() {
    let counted = tally(&[
        file("/data/a.mkv", u64::MAX, 41, 1),
        file("/data/b.mkv", u64::MAX, 42, 1),
    ]);
    assert_eq!(
        counted.logical,
        u64::MAX,
        "held at the top rather than wrapped"
    );
    assert_eq!(counted.physical, u64::MAX);
}
