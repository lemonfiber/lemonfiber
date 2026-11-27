use lemonfiber_fixtures::shelved::Shelf;

use super::holds;
use crate::ports::media::TitleKind;
use crate::walkthrough::{Shape, Why};

#[test]
fn a_library_only_stack_is_asked_a_different_question_entirely() {
    assert_eq!(Why::of(false, false).shape(), Some(Shape::LibraryOnly));
    assert!(Shape::LibraryOnly.proves().contains("already have"));
    assert!(!Shape::LibraryOnly.proves().contains("indexer"));
}

#[tokio::test]
async fn a_library_holding_one_kind_of_title_is_found_holding_it_whichever_kind_it_is() {
    for kind in TitleKind::ALL {
        assert!(holds(&Shelf::holding_only(kind), "").await, "{kind:?}");
    }
}
