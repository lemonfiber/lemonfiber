use std::path::{Path, PathBuf};

use super::Survey;
use crate::ports::filesystem::Identity;
use crate::ports::occupancy::Occupant;
use crate::space::outsized::{FLOOR, MOST};
use crate::space::Tally;

/// A file at a path, of a size, naming an underlying file shared by `links` names.
fn file(path: &str, bytes: u64, file: u64, links: u64) -> Occupant {
    Occupant {
        path: PathBuf::from(path),
        bytes,
        identity: Some(Identity { file, links }),
    }
}

/// A survey beneath `/d` of these files, holding these downloads.
fn surveyed(held: &[&str], files: Vec<Occupant>) -> Survey {
    let mut survey = Survey::beneath(Path::new("/d"), held.iter().map(|name| (*name).to_owned()));
    for occupant in files {
        survey.add(occupant);
    }
    survey
}

/// A file reachable from two directories is charged to the one first by name,
/// whichever of its names the walk reached first.
#[test]
fn a_shared_file_is_charged_to_the_first_directory_by_name() {
    let survey = surveyed(
        &[],
        vec![
            file("/d/media/film.mkv", 100, 7, 2),
            file("/d/downloads/film.mkv", 100, 7, 2),
            file("/d/media/other.mkv", 5, 8, 1),
        ],
    );
    assert_eq!(
        survey.trees(),
        vec![
            (
                "downloads".to_owned(),
                Tally {
                    logical: 100,
                    physical: 100,
                    files: 1,
                    shared: 0
                }
            ),
            (
                "media".to_owned(),
                Tally {
                    logical: 105,
                    physical: 5,
                    files: 2,
                    shared: 1
                }
            ),
        ]
    );
}

/// Two names for one file in one directory are one file, and a file whose identity
/// could not be read is charged in full.
#[test]
fn two_names_in_one_directory_are_one_file_and_an_unread_one_is_charged_in_full() {
    let survey = surveyed(
        &[],
        vec![
            file("/d/media/a.mkv", 10, 3, 2),
            file("/d/media/b.mkv", 10, 3, 2),
            Occupant {
                path: PathBuf::from("/d/loose.txt"),
                bytes: 4,
                identity: None,
            },
        ],
    );
    let trees = survey.trees();
    assert_eq!(
        trees
            .iter()
            .find(|(name, _)| name == "media")
            .map(|(_, tally)| *tally),
        Some(Tally {
            logical: 20,
            physical: 10,
            files: 2,
            shared: 1
        })
    );
    assert_eq!(
        trees
            .iter()
            .find(|(name, _)| name == "the data location itself")
            .map(|(_, tally)| tally.physical),
        Some(4)
    );
}

/// The middle file is the middle of every size, and nothing where there is none or
/// it is empty.
#[test]
fn the_typical_file_is_the_middle_one() {
    assert_eq!(surveyed(&[], Vec::new()).typical(), None);
    let three = surveyed(
        &[],
        vec![
            file("/d/a", 30, 1, 1),
            file("/d/b", 10, 2, 1),
            file("/d/c", 20, 3, 1),
        ],
    );
    assert_eq!(three.typical(), Some(20));
    let empty = surveyed(&[], vec![file("/d/a", 0, 1, 1), file("/d/b", 0, 2, 1)]);
    assert_eq!(empty.typical(), None);
}

/// Only the few largest files at or above the floor are kept, largest first.
#[test]
fn only_the_largest_files_past_the_floor_are_kept() {
    let mut files: Vec<Occupant> = (0..=u64::try_from(MOST).unwrap_or_default())
        .map(|number| {
            file(
                &format!("/d/media/big-{number}.mkv"),
                FLOOR + number,
                number + 1,
                1,
            )
        })
        .collect();
    files.push(file("/d/media/small.mkv", FLOOR - 1, 99, 1));
    let survey = surveyed(&[], files);
    let kept: Vec<u64> = survey
        .largest()
        .iter()
        .map(|occupant| occupant.bytes)
        .collect();
    assert_eq!(kept.len(), MOST);
    assert_eq!(
        kept.first().copied(),
        Some(FLOOR + u64::try_from(MOST).unwrap_or_default())
    );
    assert!(kept.windows(2).all(|pair| pair.first() >= pair.get(1)));
}

/// What a cleanup could take is kept whole, and the library around it is not.
///
/// A library of a thousand files in one folder leaves one file of it kept, the one
/// that tells an archive beside it from one that is not there to be unpacked.
#[test]
fn what_a_cleanup_could_take_is_kept_and_the_library_is_not() {
    let mut files: Vec<Occupant> = (0..1000)
        .map(|number| file(&format!("/d/media/films/{number}.mkv"), 1, number + 1, 1))
        .collect();
    files.push(file(
        "/d/downloads/held.release/held.release.mkv",
        5,
        5000,
        1,
    ));
    files.push(file("/d/downloads/unpacked/part.rar", 3, 5001, 1));
    files.push(file("/d/downloads/unpacked/film.mkv", 3, 5002, 1));
    files.push(file("/d/downloads/packed/part.r00", 3, 5003, 1));
    let survey = surveyed(&["held.release"], files);

    let holding: Vec<&Path> = survey
        .holding()
        .iter()
        .map(|occupant| occupant.path.as_path())
        .collect();
    assert_eq!(
        holding,
        vec![Path::new("/d/downloads/held.release/held.release.mkv")]
    );
    let unpacking: Vec<PathBuf> = survey
        .unpacking()
        .into_iter()
        .map(|occupant| occupant.path)
        .collect();
    assert_eq!(
        unpacking,
        vec![
            PathBuf::from("/d/downloads/packed/part.r00"),
            PathBuf::from("/d/downloads/unpacked/part.rar"),
            PathBuf::from("/d/downloads/unpacked/film.mkv"),
        ]
    );
    assert_eq!(
        survey.beside.len(),
        3,
        "one file kept for each folder, not one for each file"
    );
    assert_eq!(
        survey.sizes.len(),
        1004,
        "a size for every file, and nothing else of it"
    );
}
