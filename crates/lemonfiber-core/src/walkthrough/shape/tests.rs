use super::{Reason, Shape, Why};

#[test]
fn a_stack_that_downloads_and_can_search_gets_the_whole_walk() {
    assert_eq!(Why::of(true, true), Why::Offer(Shape::Pipeline));
    assert!(Why::of(true, true).is_offered());
}

#[test]
fn a_stack_that_acquires_nothing_is_asked_a_different_question() {
    // A media server over media the household already owns has a first-content
    // question — can Jellyfin see my files? — and it is not "shall I fetch one".
    assert_eq!(Why::of(false, false), Why::Offer(Shape::LibraryOnly));
    assert_eq!(
        Why::of(false, true),
        Why::Offer(Shape::LibraryOnly),
        "an indexer it has no use for does not change the question"
    );
}

#[test]
fn a_stack_with_nothing_to_search_is_pointed_at_the_prerequisite() {
    // Offering a walk that must stop at the first step teaches the operator that the
    // product does not know what it is doing.
    assert_eq!(Why::of(true, false), Why::Not(Reason::NoIndexers));
    assert!(!Why::of(true, false).is_offered());
    assert_eq!(Why::of(true, false).shape(), None);
}

#[test]
fn each_shape_says_what_it_sets_out_to_prove() {
    assert!(Shape::Pipeline.proves().contains("every link"));
    assert!(Shape::LibraryOnly.proves().contains("already have"));
    assert_ne!(Shape::Pipeline.proves(), Shape::LibraryOnly.proves());
}
