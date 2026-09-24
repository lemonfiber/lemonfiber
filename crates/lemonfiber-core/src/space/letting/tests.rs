use super::{agreement, offering, standing_of, Letting, WHAT_GOES};
use crate::ports::service::Seeded;
use crate::space::{Candidate, Standing, RATIO_CONSEQUENCE};

/// One completed download, as the account names it.
fn candidate(name: &str, bytes: u64, standing: Standing) -> Candidate {
    Candidate {
        name: name.to_owned(),
        bytes,
        standing,
        consequence: matches!(standing, Standing::Seeding { .. })
            .then(|| RATIO_CONSEQUENCE.to_owned()),
    }
}

/// A completed download the client is holding.
fn held(name: &str, bytes: u64, ratio: u32) -> Seeded {
    Seeded {
        name: name.to_owned(),
        bytes,
        ratio,
    }
}

/// The offer made over one seeding download.
fn offered() -> Letting {
    offering(candidate(
        "A.Show.S01E01",
        8_000,
        Standing::Seeding { ratio: 175 },
    ))
}

#[test]
fn an_offer_says_what_it_costs_and_what_goes_before_it_asks_anything() {
    let letting = offered();
    assert_eq!(
        letting.download.consequence.as_deref(),
        Some(RATIO_CONSEQUENCE)
    );
    assert_eq!(letting.goes, WHAT_GOES);
    assert!(letting.gone.is_none(), "an offer takes nothing");
}

#[test]
fn the_general_cleanups_agreement_can_never_name_one_of_these() {
    // A person agreeing to reclaim what costs nothing has not agreed to lose a
    // ratio, so the two namings are kept apart by construction rather than by
    // the two readings happening to differ.
    let letting = offered();
    let blanket = crate::agreement::over(&["A.Show.S01E01:8000"]);
    assert_ne!(letting.agreement, blanket);
    assert_ne!(letting.agreement, crate::agreement::over(&[]));
}

#[test]
fn a_ratio_that_moved_names_a_different_offer() {
    // The figure an operator weighed is in the name, so a torrent that earned
    // while they were deciding is a reading they have not seen.
    let earlier = agreement(&candidate("A", 8_000, Standing::Seeding { ratio: 175 }));
    let later = agreement(&candidate("A", 8_000, Standing::Seeding { ratio: 176 }));
    assert_ne!(earlier, later);
}

#[test]
fn what_it_occupies_and_where_it_stands_are_both_in_the_name() {
    let one = agreement(&candidate("A", 8_000, Standing::NeverImported));
    assert_ne!(
        one,
        agreement(&candidate("A", 9_000, Standing::NeverImported))
    );
    assert_ne!(one, agreement(&candidate("A", 8_000, Standing::LeftAlone)));
}

#[test]
fn a_torrent_that_took_nothing_is_named_by_the_fact_rather_than_by_the_figure() {
    // The largest figure this can carry stands for a ratio nobody can divide, and
    // naming an offer over forty-two million would read as a number.
    let infinite = agreement(&candidate(
        "A",
        8_000,
        Standing::Seeding { ratio: u32::MAX },
    ));
    assert_ne!(
        infinite,
        agreement(&candidate("A", 8_000, Standing::Seeding { ratio: 0 }))
    );
}

#[test]
fn a_download_the_account_named_is_taken_as_the_account_had_it() {
    let accounted = vec![candidate("A.Show.S01E01", 8_000, Standing::NeverImported)];
    let one = standing_of(&held("A.Show.S01E01", 8_000, 0), &accounted);
    assert_eq!(one.standing, Standing::NeverImported);
    assert!(one.consequence.is_none());
}

#[test]
fn a_download_the_walk_could_not_match_is_taken_as_still_costing_something() {
    // The safe direction, and the only one available: not finding a file is not
    // evidence that no library points at it.
    let one = standing_of(&held("A.Show.S01E01", 8_000, 42), &[]);
    assert_eq!(one.standing, Standing::Seeding { ratio: 42 });
    assert_eq!(one.consequence.as_deref(), Some(RATIO_CONSEQUENCE));
    assert_eq!(one.bytes, 8_000);
}
