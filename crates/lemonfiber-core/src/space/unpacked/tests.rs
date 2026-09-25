use std::path::{Path, PathBuf};

use super::{already_unpacked, is_archive};
use crate::ports::filesystem::Identity;
use crate::ports::occupancy::Occupant;

/// A walked file, whose identity no case here turns on.
fn file(path: &str, bytes: u64) -> Occupant {
    Occupant {
        path: PathBuf::from(path),
        bytes,
        identity: Some(Identity { file: 1, links: 1 }),
    }
}

/// The paths reported as already unpacked.
fn found(occupants: &[Occupant]) -> Vec<String> {
    already_unpacked(occupants)
        .into_iter()
        .map(|occupant| occupant.path.display().to_string())
        .collect()
}

#[test]
fn the_shapes_an_archive_part_comes_in_are_recognised() {
    for name in [
        "a.rar", "a.RAR", "a.zip", "a.7z", "a.tar", "a.gz", "a.r00", "a.r99", "a.z01", "a.001",
    ] {
        assert!(is_archive(Path::new(name)), "{name} is part of an archive");
    }
}

#[test]
fn what_is_not_an_archive_is_not_taken_for_one() {
    for name in ["a.mkv", "a.nfo", "a", "a.rare", "a.r0", "a.zed", "a.mp4"] {
        assert!(!is_archive(Path::new(name)), "{name} is not an archive");
    }
}

#[test]
fn parts_beside_what_was_unpacked_from_them_are_the_waste() {
    // In the order they were walked in, which is the order they are given in:
    // the walk above this sorts by path, so a caller gets them sorted without
    // this sorting them a second time.
    let seen = found(&[
        file("/d/A.Release/a.r00", 500),
        file("/d/A.Release/a.rar", 500),
        file("/d/A.Release/A.Release.mkv", 1_000),
    ]);
    assert_eq!(seen, ["/d/A.Release/a.r00", "/d/A.Release/a.rar"]);
}

#[test]
fn parts_with_nothing_unpacked_beside_them_are_the_only_copy() {
    // Nothing has been unpacked here yet. Removing the parts would remove the
    // whole of what was fetched, which is the opposite of reclaiming waste.
    assert!(found(&[
        file("/d/A.Release/a.rar", 500),
        file("/d/A.Release/a.r00", 500),
    ])
    .is_empty());
}

#[test]
fn a_directory_of_media_alone_holds_nothing_to_reclaim() {
    assert!(found(&[
        file("/d/films/A.Film.mkv", 1_000),
        file("/d/films/A.Film.nfo", 1),
    ])
    .is_empty());
}

#[test]
fn each_directory_is_judged_on_what_is_in_it_rather_than_on_its_neighbours() {
    // The unpacked file next door does not make the untouched archive beside it
    // safe to remove.
    let seen = found(&[
        file("/d/Done/a.rar", 500),
        file("/d/Done/Done.mkv", 1_000),
        file("/d/Waiting/b.rar", 500),
    ]);
    assert_eq!(seen, ["/d/Done/a.rar"]);
}

#[test]
fn a_file_with_no_directory_above_it_is_still_judged() {
    assert!(found(&[file("a.rar", 500)]).is_empty());
    assert_eq!(found(&[file("a.rar", 500), file("a.mkv", 5)]), ["a.rar"]);
}
