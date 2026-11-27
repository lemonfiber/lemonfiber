//! What a revocation sent `{ "dry_run": true }` answers: what removing would come to, with
//! nothing removed.

use crate::door::*;
use crate::what_a_key_admits::{code, keeping_keys, presented, sent, HERE};
use crate::what_a_push_endpoint_is::{a_machine, read, registering};

/// The rehearsal flag an answer carries.
fn rehearsed(body: &str) -> Option<bool> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .pointer("/data/rehearsed")?
        .as_bool()
}

#[tokio::test]
async fn a_rehearsed_removal_says_what_it_would_remove_and_removes_nothing() {
    let (router, token) = a_machine("rehearsed");
    let body = registering("pixel", "https://push.example/abc");
    let (registered, _) = sent(
        router.clone(),
        "POST",
        "/api/push-endpoints",
        &token,
        Some((HERE, false)),
        &body,
    )
    .await;
    assert_eq!(read(&registered.body, "/kind"), "push-endpoint");

    let rehearsing = r#"{"dry_run":true}"#;
    for _ in 0..2 {
        let (rehearsal, _) = sent(
            router.clone(),
            "DELETE",
            "/api/push-endpoints/pixel",
            &token,
            Some((HERE, false)),
            rehearsing,
        )
        .await;
        assert_eq!(read(&rehearsal.body, "/kind"), "push-endpoints");
        assert_eq!(read(&rehearsal.body, "/data/removed"), "pixel");
        assert_eq!(rehearsed(&rehearsal.body), Some(true), "{}", rehearsal.body);
    }

    let (removed, _) = sent(
        router.clone(),
        "DELETE",
        "/api/push-endpoints/pixel",
        &token,
        Some((HERE, false)),
        r#"{"dry_run":false}"#,
    )
    .await;
    assert_eq!(read(&removed.body, "/data/removed"), "pixel");
    assert_eq!(rehearsed(&removed.body), Some(false), "{}", removed.body);

    let gone = presented(
        router,
        "DELETE",
        "/api/push-endpoints/pixel",
        &token,
        Some((HERE, false)),
    )
    .await;
    assert_eq!(code(&gone), "PUSH-5", "{}", gone.body);
}

/// Whether `secret` is let in to a read from this machine.
async fn let_in(router: axum::Router, secret: &str) -> StatusCode {
    presented(
        router,
        "GET",
        "/api/explain?word=indexer",
        secret,
        Some((HERE, false)),
    )
    .await
    .status
}

#[tokio::test]
async fn a_rehearsed_revocation_leaves_the_key_letting_its_program_in() {
    let keys = keeping_keys("rehearsed-revocation");
    let admitting = housed(Recognising::knowing("a7f3"));
    let mut ctx = keeping_at(world(None, Chance::cycling()), &keys.kept);
    ctx.settings.stack_dir = keys.kept.parent().map(|dir| dir.join("stack"));
    let (router, token) = surface(ctx, &admitting);
    let (rehearsal, _) = sent(
        router.clone(),
        "DELETE",
        "/api/keys/reader",
        token.as_str(),
        Some((HERE, false)),
        r#"{"dry_run":true}"#,
    )
    .await;
    assert_eq!(read(&rehearsal.body, "/kind"), "keys");
    assert_eq!(read(&rehearsal.body, "/data/revoked"), "reader");
    assert_eq!(rehearsed(&rehearsal.body), Some(true), "{}", rehearsal.body);
    assert_eq!(
        let_in(router.clone(), keys.read.as_str()).await,
        StatusCode::OK
    );

    let revoked = presented(
        router.clone(),
        "DELETE",
        "/api/keys/reader",
        token.as_str(),
        Some((HERE, false)),
    )
    .await;
    assert_eq!(rehearsed(&revoked.body), Some(false), "{}", revoked.body);
    assert_ne!(let_in(router, keys.read.as_str()).await, StatusCode::OK);
}

#[tokio::test]
async fn a_revocation_whose_body_is_not_dry_run_alone_is_refused() {
    let (router, token) = a_machine("misread");
    for (path, body) in [
        (
            "/api/push-endpoints/pixel",
            r#"{"dry_run":true,"name":"other"}"#,
        ),
        ("/api/keys/ha", r#"{"rehearse":true}"#),
        ("/api/device-keys/phone", "not json"),
    ] {
        let (refused, _) = sent(
            router.clone(),
            "DELETE",
            path,
            &token,
            Some((HERE, false)),
            body,
        )
        .await;
        assert_eq!(code(&refused), "ASK-14", "{path}: {}", refused.body);
    }
}
