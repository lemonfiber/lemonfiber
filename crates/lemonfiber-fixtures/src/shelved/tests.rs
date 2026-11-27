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

/// A library holding titles of one kind alone holds that kind and no other.
#[tokio::test]
async fn a_library_holding_one_kind_holds_that_kind_alone() {
    let shelf = Shelf::holding_only(TitleKind::Music);
    assert!(shelf
        .has_item(TitleKind::Music, "anything")
        .await
        .is_ok_and(|held| held));
    assert!(shelf
        .has_item(TitleKind::Books, "anything")
        .await
        .is_ok_and(|held| !held));
}
