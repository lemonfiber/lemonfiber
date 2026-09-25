use std::path::{Path, PathBuf};

use super::{beside, ours};
use crate::ports::occupancy::Occupant;

/// The data location every case here is about.
fn root() -> &'static Path {
    Path::new("/srv/media")
}

/// The media types a stack running television and film declares.
fn types() -> Vec<String> {
    vec!["tv".to_owned(), "movies".to_owned()]
}

/// A walked file at a path beneath the data location.
fn file(path: &str, bytes: u64) -> Occupant {
    Occupant {
        path: root().join(path),
        bytes,
        identity: None,
    }
}

#[test]
fn the_stacks_own_directories_are_the_downloads_and_one_per_media_type() {
    assert_eq!(
        ours(&types()),
        vec![
            "downloads".to_owned(),
            "media/movies".to_owned(),
            "media/tv".to_owned()
        ]
    );
}

/// A media type the stack does not declare is not one of ours, which is what
/// keeps a directory the operator made under `media` from being removed.
#[test]
fn a_stack_that_manages_nothing_owns_only_the_downloads() {
    assert_eq!(ours(&[]), vec!["downloads".to_owned()]);
}

#[test]
fn what_the_stack_wrote_is_not_reported_as_somebody_elses() {
    let found = beside(
        root(),
        &[
            file("downloads/A.Show/a.mkv", 10),
            file("media/tv/A Show/S01E01.mkv", 20),
            file("media/movies/A Film/film.mkv", 30),
        ],
        &types(),
    );

    assert!(found.is_empty(), "{found:?}");
}

/// The finding this exists for: a directory the operator put beside the library.
#[test]
fn a_directory_the_operator_put_there_is_found_and_counted_as_one_thing() {
    let found = beside(
        root(),
        &[
            file("Photographs/2019/a.jpg", 100),
            file("Photographs/2020/b.jpg", 200),
            file("media/tv/A Show/S01E01.mkv", 20),
        ],
        &types(),
    );

    assert_eq!(found.len(), 1, "{found:?}");
    let one = found.first().cloned();
    assert_eq!(
        one.as_ref().map(|one| one.at.clone()),
        Some("Photographs".to_owned())
    );
    assert_eq!(one.as_ref().map(|one| one.files), Some(2));
    assert_eq!(one.map(|one| one.bytes), Some(300));
}

/// A directory under `media` that no service was pointed at is somebody else's,
/// which is the case a rule keyed on the top-level directory alone would miss.
#[test]
fn a_library_beside_the_ones_the_stack_manages_is_somebody_elses() {
    let found = beside(
        root(),
        &[file("media/home-video/wedding.mp4", 500)],
        &types(),
    );

    assert_eq!(
        found
            .iter()
            .map(|one| one.at.clone())
            .collect::<Vec<String>>(),
        vec!["media/home-video".to_owned()]
    );
}

/// A file loose in the data location has no directory to be credited to, so it
/// is named by itself rather than swallowed into the root.
#[test]
fn a_file_loose_in_the_data_location_is_named_by_itself() {
    let found = beside(root(), &[file("notes.txt", 12)], &types());

    assert_eq!(
        found
            .iter()
            .map(|one| one.at.clone())
            .collect::<Vec<String>>(),
        vec!["notes.txt".to_owned()]
    );
}

/// A walk that answered about somewhere else is not evidence about here.
#[test]
fn a_file_outside_the_data_location_is_not_counted_against_it() {
    let found = beside(
        root(),
        &[Occupant {
            path: PathBuf::from("/elsewhere/a.mkv"),
            bytes: 40,
            identity: None,
        }],
        &types(),
    );

    assert!(found.is_empty(), "{found:?}");
}

/// Two findings are two findings, ordered so the report reads the same twice.
#[test]
fn several_findings_are_reported_in_a_settled_order() {
    let found = beside(
        root(),
        &[file("Zips/a.zip", 1), file("Archive/b.tar", 2)],
        &types(),
    );

    assert_eq!(
        found
            .iter()
            .map(|one| one.at.clone())
            .collect::<Vec<String>>(),
        vec!["Archive".to_owned(), "Zips".to_owned()]
    );
}
