use super::{reading, steps, A_FILTER_NOT_A_LOCK};

/// No limit, the youngest step and an ordinary age each read as their own words.
#[test]
fn each_limit_reads_as_the_words_for_it() {
    assert_eq!(reading(None), "anything");
    assert_eq!(reading(Some(0)), "only what suits everyone");
    assert_eq!(reading(Some(15)), "nothing above about 15");
}

/// A limit that is none of the steps offered still reads as something true.
///
/// An account may carry one set in the media server's own screens, and a reader
/// shown nothing would take that for an account with no limit on it at all.
#[test]
fn a_limit_that_is_not_a_step_offered_still_reads() {
    assert!(
        !steps().iter().any(|step| step.age == 13),
        "13 is a step offered, so this asserts nothing"
    );
    assert_eq!(reading(Some(13)), "nothing above about 13");
}

/// The steps rise, so a list of them reads as a ladder rather than as a set of
/// unrelated numbers.
#[test]
fn the_steps_rise_from_the_youngest_audience() {
    let ages: Vec<u32> = steps().iter().map(|step| step.age).collect();

    assert!(ages.len() > 2, "there is no ladder to read: {ages:?}");
    assert_eq!(
        ages.first(),
        Some(&0),
        "the ladder does not start at nought"
    );
    assert!(ages.is_sorted(), "the steps do not rise: {ages:?}");
}

/// The one thing said about what a limit is says both halves: what it does, and
/// that it is not a lock.
#[test]
fn what_a_limit_is_says_what_it_is_not() {
    assert!(A_FILTER_NOT_A_LOCK.contains("content filter"));
    assert!(A_FILTER_NOT_A_LOCK.contains("not a "));
    assert!(
        A_FILTER_NOT_A_LOCK.contains("security boundary"),
        "{A_FILTER_NOT_A_LOCK}"
    );
}

/// Every step says who it suits, or a list of them is a column of numbers with a
/// blank beside each.
#[test]
fn every_step_says_who_it_suits() {
    for step in steps() {
        // Bound rather than called in the message, which only runs on failure.
        let said = reading(Some(step.age));
        assert!(!step.suits.is_empty(), "{said} has nothing beside it");
    }
}
