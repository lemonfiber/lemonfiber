use super::*;

/// A library rescans whenever asked, and answers whether it holds a title as scripted.
#[tokio::test]
async fn a_library_rescans_and_answers_as_scripted() {
    assert!(Shelf::holding(Some(true)).rescan().await.is_ok());
    assert!(Shelf::holding(Some(true))
        .has_item(TitleKind::Tv, "anything")
        .await
        .is_ok_and(|held| held));
    assert!(matches!(
        Shelf::holding(None)
            .has_item(TitleKind::Movies, "anything")
            .await,
        Err(Failure::Unavailable { .. })
    ));
}
