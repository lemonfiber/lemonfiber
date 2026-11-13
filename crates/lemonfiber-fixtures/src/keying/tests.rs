use super::*;

/// Every key question but `astray` is recorded and answered as a server holding nothing,
/// and an outside account is refused, so a check that reaches one says so.
#[tokio::test]
async fn every_key_question_is_recorded_and_holds_nothing() {
    let keys = Keys::answering(&[]);

    assert!(keys
        .filed_as("gate")
        .await
        .is_ok_and(|filed| filed.is_empty()));
    assert!(keys.dated("gate").await.is_ok_and(|dated| dated.is_empty()));
    assert!(keys.mint("gate").await.is_ok_and(|key| key.is_empty()));
    assert!(keys.answers_to("k").await.is_ok());
    assert!(keys.revoke("k").await.is_ok());
    assert!(matches!(
        keys.account("gate").await,
        Err(Failure::Unsupported { .. })
    ));
    assert!(matches!(
        keys.listed_as().await,
        Err(Failure::Unsupported { .. })
    ));
    assert_eq!(
        keys.calls(),
        [
            "filed_as gate",
            "dated gate",
            "mint gate",
            "answers_to k",
            "revoke k"
        ]
    );
}
