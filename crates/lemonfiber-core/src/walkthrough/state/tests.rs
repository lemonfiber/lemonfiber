use super::State;
use super::Step;

#[test]
fn every_step_of_the_walk_has_a_state_to_report_it_by() {
    for step in Step::all() {
        let state = State::of_step(step);
        // A step that is being worked on is either running or one of the two ends.
        assert!(
            state.is_running() || matches!(state, State::Offered | State::Complete),
            "{step:?} reported as {state:?}"
        );
    }
}

#[test]
fn reaching_the_last_step_is_being_complete() {
    assert_eq!(State::of_step(Step::Available), State::Complete);
    assert_eq!(State::of_step(Step::Choosing), State::Offered);
    // Filing what arrived is still importing as far as the operator is concerned.
    assert_eq!(State::of_step(Step::Scanning), State::Importing);
}

#[test]
fn only_a_failure_is_a_problem() {
    let problems: Vec<State> = State::all()
        .into_iter()
        .filter(|state| state.is_a_problem())
        .collect();
    assert_eq!(
        problems,
        vec![State::Failed],
        "declining and leaving cost nothing by design"
    );
}

#[test]
fn a_walk_is_settled_once_it_stops_owing_the_operator_anything() {
    let unsettled: Vec<State> = State::all()
        .into_iter()
        .filter(|state| !state.is_settled())
        .collect();
    assert_eq!(
        unsettled,
        vec![
            State::Offered,
            State::Searching,
            State::Grabbing,
            State::Downloading,
            State::Importing
        ]
    );
}

#[test]
fn every_state_has_the_word_the_specification_names_it_by() {
    let words: Vec<&str> = State::all().into_iter().map(State::word).collect();
    assert_eq!(
        words,
        vec![
            "offered",
            "skipped",
            "searching",
            "grabbing",
            "downloading",
            "importing",
            "complete",
            "failed",
            "abandoned"
        ]
    );
}
