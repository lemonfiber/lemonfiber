use crate::walkthrough::{Reason, Shape, Why};

#[test]
fn what_a_stack_is_offered_follows_from_what_it_can_do() {
    // The offer is the pure decision; what is asked of the services to reach it is
    // two booleans, and the mapping between them is proven where it is written.
    assert_eq!(Why::of(true, true), Why::Offer(Shape::Pipeline));
    assert_eq!(Why::of(false, false), Why::Offer(Shape::LibraryOnly));
    assert_eq!(Why::of(true, false), Why::Not(Reason::NoIndexers));
}
