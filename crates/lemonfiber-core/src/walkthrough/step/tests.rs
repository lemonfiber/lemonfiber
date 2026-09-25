use super::{Link, Step};
use crate::trace::Stage;

#[test]
fn the_steps_are_declared_in_the_order_they_happen() {
    let mut walked = Step::all().to_vec();
    walked.sort_unstable();
    assert_eq!(
        walked,
        Step::all().to_vec(),
        "declaration order is walk order"
    );
    assert!(Step::Choosing < Step::Available);
    assert!(Step::all().last().copied().is_some_and(Step::is_the_end));
}

#[test]
fn only_the_last_step_means_it_worked() {
    let ends: Vec<Step> = Step::all().into_iter().filter(|s| s.is_the_end()).collect();
    assert_eq!(ends, vec![Step::Available]);
}

#[test]
fn every_pipeline_stage_lands_on_a_step() {
    // The live walk and the after-the-fact trace have to agree on where something
    // got to, so every stage the trace knows maps onto a step the walk narrates.
    let stages = [
        (Stage::NotMonitored, Step::Choosing),
        (Stage::Monitored, Step::Choosing),
        (Stage::Searching, Step::Searching),
        (Stage::Found, Step::Searching),
        (Stage::Grabbed, Step::Grabbing),
        (Stage::Downloading, Step::Downloading),
        (Stage::Downloaded, Step::Downloading),
        (Stage::Importing, Step::Importing),
        (Stage::Imported, Step::Scanning),
        (Stage::Available, Step::Available),
    ];
    for (stage, step) in stages {
        assert_eq!(Step::of_stage(stage), step, "{stage:?}");
    }
}

#[test]
fn the_stage_mapping_never_goes_backwards() {
    // A later stage can never map to an earlier step, or a walk would appear to
    // retreat while the pipeline advanced.
    let ordered = [
        Stage::NotMonitored,
        Stage::Monitored,
        Stage::Searching,
        Stage::Found,
        Stage::Grabbed,
        Stage::Downloading,
        Stage::Downloaded,
        Stage::Importing,
        Stage::Imported,
        Stage::Available,
    ];
    for pair in ordered.windows(2) {
        let (earlier, later) = (pair.first().copied(), pair.last().copied());
        let steps = earlier
            .zip(later)
            .map(|(a, b)| (Step::of_stage(a), Step::of_stage(b)));
        assert!(steps.is_some_and(|(a, b)| a <= b), "{pair:?}");
    }
}

#[test]
fn every_step_says_what_it_is_doing_and_who_is_doing_it() {
    for step in Step::all() {
        assert!(!step.said().is_empty(), "{step:?}");
        assert!(step.done_by().starts_with("the"), "{step:?}");
        // Present tense and unfinished: it is read while it runs.
        assert!(!step.said().ends_with('.'), "{step:?}");
    }
}

#[test]
fn a_copy_is_explained_and_a_hardlink_is_not_a_problem() {
    assert!(Link::Copied.consequence().contains("twice"));
    assert!(
        Link::Copied.remedy().is_some(),
        "a copy has something to do about it"
    );
    assert!(Link::Hardlinked.consequence().contains("no extra disk"));
    assert_eq!(
        Link::Hardlinked.remedy(),
        None,
        "nothing to fix when it worked"
    );
}
