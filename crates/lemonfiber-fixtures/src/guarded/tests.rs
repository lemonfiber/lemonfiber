use super::*;

/// A guard enforces nothing it is asked about and presents an empty certificate.
#[tokio::test]
async fn a_guard_holds_nothing_and_presents_an_empty_certificate() {
    let kept = Kept::default();

    assert!(kept.guarding().await.is_ok_and(|held| held.is_none()));
    assert!(kept
        .certificate()
        .await
        .is_ok_and(|presented| presented.fingerprint.is_empty()));
    assert!(kept.handed().is_empty());
}
