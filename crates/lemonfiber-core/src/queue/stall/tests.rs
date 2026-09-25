use super::Stall;

#[test]
fn every_category_says_what_it_is_what_causes_it_what_it_costs_and_what_to_do() {
    // A category that cannot answer all four is a status line wearing a name,
    // which is the thing this exists instead of.
    for stall in Stall::ALL {
        assert!(!stall.word().is_empty(), "{stall:?}");
        assert!(!stall.typically().is_empty(), "{stall:?}");
        assert!(!stall.means().is_empty(), "{stall:?}");
        assert!(!stall.remedies().is_empty(), "{stall:?}");
    }
}

#[test]
fn no_two_categories_offer_the_same_advice() {
    // If two did, they would not be two categories — they would be one with a
    // spelling difference, and the operator would learn the distinction is
    // decorative.
    let mut first: Vec<String> = Stall::ALL
        .iter()
        .map(|stall| stall.first_remedy())
        .collect();
    let count = first.len();
    first.sort_unstable();
    first.dedup();
    assert_eq!(
        first.len(),
        count,
        "two categories give the same first advice"
    );
}

#[test]
fn the_worst_thing_sorts_first() {
    // Declaration order is the ranking, so a summary leading with the worst
    // needs no second ordering to keep in step with this one.
    let mut shuffled = vec![Stall::Slow, Stall::RedownloadLoop, Stall::StalledDownload];
    shuffled.sort_unstable();
    assert_eq!(
        shuffled,
        vec![Stall::RedownloadLoop, Stall::StalledDownload, Stall::Slow]
    );
}

#[test]
fn something_still_arriving_is_not_worth_an_interruption() {
    // An alert about something that is working is how the whole check gets
    // muted, and the leak alert with it.
    assert!(!Stall::Slow.wants_attention());
    for stall in Stall::ALL.iter().filter(|stall| **stall != Stall::Slow) {
        assert!(stall.wants_attention(), "{stall:?}");
    }
}
