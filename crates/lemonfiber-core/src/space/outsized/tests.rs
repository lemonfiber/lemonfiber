use std::path::PathBuf;

use super::{outsized, FLOOR};
use crate::ports::occupancy::Occupant;

/// A walked file of a given size, whose identity no case here turns on.
fn file(path: &str, bytes: u64) -> Occupant {
    Occupant {
        path: PathBuf::from(path),
        bytes,
        identity: None,
    }
}

/// A library of ordinary files, each a twentieth of the floor.
///
/// Enough of them that a handful of outsized ones does not become the middle
/// file itself — which is the trap the comparison is arranged around, and one a
/// case about the cap can walk into by adding twenty of them.
fn ordinary() -> Vec<Occupant> {
    many_ordinary(9)
}

/// The same, however many are asked for.
fn many_ordinary(count: u32) -> Vec<Occupant> {
    (0..count)
        .map(|number| file(&format!("/d/films/ordinary-{number:03}.mkv"), FLOOR / 20))
        .collect()
}

#[test]
fn one_file_an_order_of_magnitude_out_is_pointed_at() {
    let mut walk = ordinary();
    walk.push(file("/d/films/A.Remux.mkv", FLOOR * 2));
    let found = outsized(&walk);
    assert_eq!(found.len(), 1);
    let pointed = found.first();
    assert!(
        pointed.is_some_and(|one| one.path == "/d/films/A.Remux.mkv"
            && one.bytes == FLOOR * 2
            && one.times_typical >= 8),
        "{pointed:?}"
    );
}

#[test]
fn a_library_whose_files_are_all_large_reports_none_of_them() {
    // Large is a comparison. A collection of remuxes is not misconfigured
    // because its files are large.
    let walk: Vec<Occupant> = (0..9)
        .map(|number| file(&format!("/d/films/remux-{number}.mkv"), FLOOR * 2))
        .collect();
    assert!(outsized(&walk).is_empty());
}

#[test]
fn a_small_collections_largest_file_is_not_an_anomaly() {
    // Far out of line with the rest, and still nothing worth saying: without
    // the floor this line would fire on every tidy directory there is.
    let walk = vec![
        file("/d/a.nfo", 1),
        file("/d/b.nfo", 1),
        file("/d/c.mkv", 1_000_000),
    ];
    assert!(outsized(&walk).is_empty());
}

#[test]
fn the_middle_file_is_what_the_comparison_is_against_rather_than_the_average() {
    // One enormous file drags an average towards itself, which is how an
    // outlier comes to look ordinary. Two of them, and the mean of this walk
    // would be over the floor while the middle stays where it belongs.
    let mut walk = ordinary();
    walk.push(file("/d/films/One.mkv", FLOOR * 40));
    walk.push(file("/d/films/Two.mkv", FLOOR * 40));
    let found = outsized(&walk);
    assert_eq!(found.len(), 2, "both are still out of line");
}

#[test]
fn nothing_is_reported_where_there_is_nothing_to_compare_against() {
    assert!(outsized(&[]).is_empty());
    assert!(
        outsized(&[
            file("/d/a.mkv", FLOOR * 4),
            file("/d/b", 0),
            file("/d/c", 0)
        ])
        .is_empty(),
        "a walk whose middle file is empty gives no ratio to compare against"
    );
}

#[test]
fn a_handful_is_a_highlight_and_a_hundred_is_a_listing() {
    // Forty-one ordinary files against seven outsized ones, so the middle file
    // is still an ordinary one: a walk whose middle is itself outsized has
    // nothing out of line in it by definition, and would report none.
    let mut walk = many_ordinary(41);
    for number in 0..7 {
        walk.push(file(&format!("/d/films/big-{number:02}.mkv"), FLOOR * 2));
    }
    let found = outsized(&walk);
    assert_eq!(found.len(), 5);
    let first = found.first();
    assert!(
        first.is_some_and(|one| one.path == "/d/films/big-00.mkv"),
        "a tie is broken by name, so two runs read alike: {first:?}"
    );
}

#[test]
fn the_largest_is_read_first() {
    let mut walk = ordinary();
    walk.push(file("/d/films/Bigger.mkv", FLOOR * 4));
    walk.push(file("/d/films/Big.mkv", FLOOR * 2));
    let found = outsized(&walk);
    let order: Vec<&str> = found.iter().map(|one| one.path.as_str()).collect();
    assert_eq!(order, ["/d/films/Bigger.mkv", "/d/films/Big.mkv"]);
}
