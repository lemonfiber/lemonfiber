use super::{offer_setup, Step};

#[test]
fn every_step_is_called_something() {
    // A step with no words is one a surface would present as a blank heading,
    // and the walk has fourteen of them: the one that was forgotten is the one
    // nobody sees until they reach it.
    let unnamed: Vec<Step> = Step::ORDER
        .into_iter()
        .filter(|step| step.label().is_empty())
        .collect();
    assert!(unnamed.is_empty(), "{unnamed:?}");
}

#[test]
fn no_two_steps_are_called_the_same_thing() {
    let mut labels: Vec<&str> = Step::ORDER.into_iter().map(Step::label).collect();
    labels.sort_unstable();
    let count = labels.len();
    labels.dedup();
    assert_eq!(labels.len(), count);
}

#[test]
fn setup_is_offered_exactly_where_there_is_nothing_configured() {
    assert!(offer_setup(false));
    assert!(!offer_setup(true));
}
