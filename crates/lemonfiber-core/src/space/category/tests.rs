use super::{Category, Consumption, Reclaim};
use crate::space::tally::Tally;

/// Every category there is, so a rule about all of them reads all of them.
fn every() -> Vec<Category> {
    vec![
        Category::Tree("films".to_owned()),
        Category::Landing,
        Category::Seeding,
        Category::Orphaned,
        Category::Extracted,
        Category::Services,
        Category::Unmanaged,
    ]
}

#[test]
fn every_category_is_headed_and_says_what_reclaiming_it_costs() {
    let categories = every();
    assert_eq!(categories.len(), 7, "every line the accounting can carry");
    for category in categories {
        let heading = category.heading();
        let says = category.reclaim().says();
        assert!(!heading.is_empty());
        assert!(says.len() > 15, "{heading} says what it costs: {says}");
    }
}

#[test]
fn a_tree_is_headed_by_the_name_the_operator_gave_it() {
    assert_eq!(Category::Tree("films".to_owned()).heading(), "films");
    assert_eq!(
        Category::Tree("films".to_owned()).reclaim(),
        Reclaim::ByLosingContent
    );
}

#[test]
fn only_what_costs_nothing_is_offered_to_be_reclaimed() {
    // The two that are reclaimable and are not offered are the point: a
    // torrent's removal is weighed against a tracker's opinion, and something
    // the operator asked to be left alone is not this product's to take.
    assert!(Reclaim::TheEasyWin.offered());
    assert!(Reclaim::AlreadyHaveIt.offered());
    for cost in [
        Reclaim::AtTheCostOfRatio,
        Reclaim::YouSaidNot,
        Reclaim::ByLosingContent,
        Reclaim::InProgress,
        Reclaim::Marginally,
    ] {
        assert!(!cost.offered(), "{} is never taken unasked", cost.says());
    }
}

#[test]
fn what_is_left_alone_says_the_operator_asked_for_it() {
    let said = Reclaim::YouSaidNot.says();
    assert!(said.contains("left alone"), "{said}");
    assert!(said.starts_with("no"), "{said}");
}

#[test]
fn a_line_takes_its_cost_from_what_it_is_about() {
    let line = Consumption::of(
        Category::Seeding,
        Tally {
            logical: 10,
            physical: 10,
            files: 1,
            shared: 0,
        },
    );
    assert_eq!(line.reclaim, Reclaim::AtTheCostOfRatio);
    assert!(line.reclaim.says().contains("ratio"));
    assert!(line.any());
}

#[test]
fn a_line_accounting_for_nothing_is_left_out() {
    let empty = Consumption::of(Category::Orphaned, Tally::default());
    assert!(!empty.any());
    let counted = Consumption::of(
        Category::Orphaned,
        Tally {
            logical: 0,
            physical: 0,
            files: 3,
            shared: 0,
        },
    );
    assert!(
        counted.any(),
        "three empty files are still three files somebody may want gone"
    );
}
