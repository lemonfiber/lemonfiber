use async_trait::async_trait;

use super::holds;
use crate::ports::media::TitleKind;
use crate::ports::service::{Failure, Library};
use crate::walkthrough::{Shape, Why};

/// A library holding titles of one kind alone.
struct Holding(TitleKind);

#[async_trait]
impl Library for Holding {
    async fn has_item(&self, kind: TitleKind, _term: &str) -> Result<bool, Failure> {
        Ok(kind == self.0)
    }

    async fn rescan(&self) -> Result<(), Failure> {
        Ok(())
    }
}

#[test]
fn a_library_only_stack_is_asked_a_different_question_entirely() {
    assert_eq!(Why::of(false, false).shape(), Some(Shape::LibraryOnly));
    assert!(Shape::LibraryOnly.proves().contains("already have"));
    assert!(!Shape::LibraryOnly.proves().contains("indexer"));
}

#[tokio::test]
async fn a_library_holding_one_kind_of_title_is_found_holding_it_whichever_kind_it_is() {
    for kind in TitleKind::ALL {
        assert!(holds(&Holding(kind), "").await, "{kind:?}");
    }
}
