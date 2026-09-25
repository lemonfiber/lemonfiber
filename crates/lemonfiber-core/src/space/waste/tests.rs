use std::collections::BTreeSet;
use std::path::PathBuf;

use super::{candidates, ratio_reads, Standing, RATIO_CONSEQUENCE};
use crate::ports::filesystem::Identity;
use crate::ports::occupancy::Occupant;
use crate::ports::service::Seeded;

/// A completed download the client is holding.
fn held(name: &str, bytes: u64, ratio: u32) -> Seeded {
    Seeded {
        name: name.to_owned(),
        bytes,
        ratio,
    }
}

/// A walked file with a given number of names pointing at it.
fn file(path: &str, bytes: u64, inode: u64, links: u64) -> Occupant {
    Occupant {
        path: PathBuf::from(path),
        bytes,
        identity: Some(Identity { file: inode, links }),
    }
}

/// Nothing has been marked to be left alone.
fn nothing_marked() -> BTreeSet<String> {
    BTreeSet::new()
}

/// The one download in these cases has been.
fn marked() -> BTreeSet<String> {
    BTreeSet::from(["A.Show.S01E01".to_owned()])
}

#[test]
fn a_download_with_one_name_was_never_imported() {
    // One name means nothing ever linked it into a library, which is the
    // evidence — not a guess from the client having finished with it.
    let found = candidates(
        &[held("A.Show.S01E01", 8_000, 0)],
        &BTreeSet::new(),
        &nothing_marked(),
        &[file(
            "/srv/media/downloads/A.Show.S01E01/a.mkv",
            8_000,
            41,
            1,
        )],
    );
    assert_eq!(found.len(), 1);
    let one = found.first();
    assert!(
        one.is_some_and(|candidate| candidate.standing == Standing::NeverImported
            && candidate.offered()
            && candidate.consequence.is_none()),
        "nothing is lost by removing it, so nothing is said about weighing it: {one:?}"
    );
}

#[test]
fn a_download_with_a_second_name_was_imported_and_is_seeding() {
    let found = candidates(
        &[held("A.Show.S01E01", 8_000, 175)],
        &BTreeSet::new(),
        &nothing_marked(),
        &[file(
            "/srv/media/downloads/A.Show.S01E01/a.mkv",
            8_000,
            41,
            2,
        )],
    );
    let one = found.first();
    assert!(
        one.is_some_and(
            |candidate| candidate.standing == Standing::Seeding { ratio: 175 }
                && !candidate.offered()
                && candidate.consequence.as_deref() == Some(RATIO_CONSEQUENCE)
        ),
        "a consequence outside this machine is not this product's to weigh: {one:?}"
    );
}

#[test]
fn what_removing_a_seeding_torrent_costs_is_said_in_what_it_does() {
    let said = RATIO_CONSEQUENCE.to_lowercase();
    assert!(said.contains("stops it seeding"), "{said}");
    assert!(said.contains("ratio"), "{said}");
    assert!(
        said.contains("account"),
        "what losing the ratio actually costs: {said}"
    );
}

#[test]
fn a_download_a_service_is_still_waiting_for_is_never_called_waste() {
    // One name and an *arr still queued for it is an import that has not
    // happened yet, not one that never will.
    let awaited = BTreeSet::from(["A.Show.S01E01".to_owned()]);
    let found = candidates(
        &[held("A.Show.S01E01", 8_000, 20)],
        &awaited,
        &nothing_marked(),
        &[file(
            "/srv/media/downloads/A.Show.S01E01/a.mkv",
            8_000,
            41,
            1,
        )],
    );
    let one = found.first();
    assert!(
        one.is_some_and(|candidate| !candidate.offered()
            && candidate.standing == Standing::Seeding { ratio: 20 }),
        "{one:?}"
    );
}

#[test]
fn what_the_operator_asked_to_be_left_alone_is_never_offered() {
    let found = candidates(
        &[held("A.Show.S01E01", 8_000, 0)],
        &BTreeSet::new(),
        &marked(),
        &[file(
            "/srv/media/downloads/A.Show.S01E01/a.mkv",
            8_000,
            41,
            1,
        )],
    );
    let one = found.first();
    assert!(
        one.is_some_and(|candidate| candidate.standing == Standing::LeftAlone
            && !candidate.offered()
            && candidate
                .consequence
                .as_deref()
                .is_some_and(|said| said.contains("left alone"))),
        "it says whose instruction it is following: {one:?}"
    );
}

#[test]
fn a_ratio_reads_as_the_client_would_write_it() {
    assert_eq!(ratio_reads(175).as_deref(), Some("1.75"));
    assert_eq!(ratio_reads(0).as_deref(), Some("0.00"));
    assert_eq!(ratio_reads(7).as_deref(), Some("0.07"));
    assert_eq!(ratio_reads(1_000).as_deref(), Some("10.00"));
    assert_eq!(
        ratio_reads(u32::MAX),
        None,
        "a torrent that downloaded nothing has no ratio to divide"
    );
}

#[test]
fn a_download_no_file_could_be_matched_to_is_left_out_rather_than_guessed_at() {
    // "I could not find it" and "nothing points at it" must not read alike,
    // because one of them is a reason to delete something.
    let found = candidates(
        &[held("A.Show.S01E01", 8_000, 0)],
        &BTreeSet::new(),
        &nothing_marked(),
        &[file("/srv/media/films/Something.Else.mkv", 8_000, 41, 1)],
    );
    assert!(found.is_empty());
}

#[test]
fn a_download_named_as_the_file_itself_is_matched_by_its_stem() {
    let found = candidates(
        &[held("A.Film.2019", 9_000, 0)],
        &BTreeSet::new(),
        &nothing_marked(),
        &[file("/srv/media/downloads/A.Film.2019.mkv", 9_000, 42, 1)],
    );
    assert_eq!(found.len(), 1);
    assert!(found
        .first()
        .is_some_and(|one| one.standing == Standing::NeverImported));
}

#[test]
fn a_file_the_platform_would_not_identify_is_not_evidence_of_a_link() {
    let unidentified = Occupant {
        path: PathBuf::from("/srv/media/downloads/A.Show.S01E01/a.mkv"),
        bytes: 8_000,
        identity: None,
    };
    let found = candidates(
        &[held("A.Show.S01E01", 8_000, 0)],
        &BTreeSet::new(),
        &nothing_marked(),
        &[unidentified],
    );
    let one = found.first();
    assert!(
        one.is_some_and(|candidate| candidate.standing == Standing::NeverImported),
        "no second name was found, and absence of a reading is not a link: {one:?}"
    );
}

#[test]
fn the_largest_is_read_first_and_a_tie_is_broken_by_name() {
    let found = candidates(
        &[
            held("Bee", 1_000, 0),
            held("Cee", 9_000, 0),
            held("Ape", 1_000, 0),
        ],
        &BTreeSet::new(),
        &nothing_marked(),
        &[
            file("/d/Bee/a.mkv", 1_000, 1, 1),
            file("/d/Cee/a.mkv", 9_000, 2, 1),
            file("/d/Ape/a.mkv", 1_000, 3, 1),
        ],
    );
    let order: Vec<&str> = found.iter().map(|one| one.name.as_str()).collect();
    assert_eq!(order, ["Cee", "Ape", "Bee"]);
}
