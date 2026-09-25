use crate::walkthrough::{Shape, Why};

#[test]
fn a_library_only_stack_is_asked_a_different_question_entirely() {
    assert_eq!(Why::of(false, false).shape(), Some(Shape::LibraryOnly));
    assert!(Shape::LibraryOnly.proves().contains("already have"));
    assert!(!Shape::LibraryOnly.proves().contains("indexer"));
}
