use super::{Shape, State, Step, LARGE};

#[test]
fn the_walk_is_offered_before_it_is_anything_else() {
    // The first state is the offer, because the operator's first involvement is
    // being asked — not being walked.
    assert_eq!(State::default(), State::Offered);
    assert_eq!(Step::default(), Step::Choosing);
    assert_eq!(Shape::default(), Shape::Pipeline);
}

#[test]
fn a_size_worth_stating_before_a_wait_is_gigabytes() {
    // Below this the wait is short enough that a figure is noise rather than warning.
    assert_eq!(LARGE / (1024 * 1024 * 1024), 4);
}
