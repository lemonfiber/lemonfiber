//! The routes the phones that asked to be woken are kept through.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use lemonfiber_fixtures::ports::Chance;
use ring::agreement::{EphemeralPrivateKey, ECDH_P256};
use ring::rand::SystemRandom;

use crate::door::*;
use crate::what_a_key_admits::{code, presented, sent, HERE, NEXT_DOOR};

/// A machine keeping its files beneath a directory named for `named`, and its own token.
fn a_machine(named: &str) -> (axum::Router, String) {
    let admission = keeping(&format!("push-{named}"));
    let mut ctx = world(Some(admission.clone()), Chance::exactly(None));
    let env = a_directory(&format!("push-env-{named}")).join(".env");
    if let Some(dir) = env.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    ctx.settings.env_file = Some(env.clone());
    ctx.settings.stack_dir = env.parent().map(|dir| dir.join("stack"));
    let admitting = Arc::new(Admitting {
        kept: Some(admission),
        ..Admitting::default()
    });
    let (router, token) = surface(ctx, &admitting);
    (router, token.as_str().to_owned())
}

/// A registration under `name` at `endpoint`, as a phone asks it.
fn registering(name: &str, endpoint: &str) -> String {
    let p256dh = EphemeralPrivateKey::generate(&ECDH_P256, &SystemRandom::new())
        .and_then(|key| key.compute_public_key())
        .map(|public| URL_SAFE_NO_PAD.encode(public.as_ref()))
        .unwrap_or_default();
    serde_json::json!({
        "name": name,
        "endpoint": endpoint,
        "p256dh": p256dh,
        "auth": URL_SAFE_NO_PAD.encode([9_u8; 16]),
    })
    .to_string()
}

/// The value at `pointer` in an answer's body.
fn read(body: &str, pointer: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|reply| {
            reply
                .pointer(pointer)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

/// An endpoint registered is listed without its keys, and removed by name.
#[tokio::test]
async fn an_endpoint_is_registered_listed_and_removed() {
    let (router, token) = a_machine("kept");
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
    assert_eq!(
        read(&registered.body, "/kind"),
        "push-endpoint",
        "{}",
        registered.body
    );

    let listed = presented(
        router.clone(),
        "GET",
        "/api/push-endpoints",
        &token,
        Some((HERE, false)),
    )
    .await;
    assert_eq!(read(&listed.body, "/kind"), "push-endpoints");
    assert_eq!(read(&listed.body, "/data/endpoints/0/name"), "pixel");
    assert!(!listed.body.contains("auth"), "{}", listed.body);
    assert!(!listed.body.contains("p256dh"), "{}", listed.body);

    let (again, _) = sent(
        router.clone(),
        "POST",
        "/api/push-endpoints",
        &token,
        Some((HERE, false)),
        &body,
    )
    .await;
    assert_eq!(code(&again), "PUSH-2", "{}", again.body);

    let removed = presented(
        router.clone(),
        "DELETE",
        "/api/push-endpoints/pixel",
        &token,
        Some((HERE, false)),
    )
    .await;
    assert_eq!(read(&removed.body, "/data/removed"), "pixel");

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

/// An address that is not https is refused, and so is a request that is not a registration.
#[tokio::test]
async fn what_is_not_a_registration_is_refused() {
    let (router, token) = a_machine("refused");
    let (plain, _) = sent(
        router.clone(),
        "POST",
        "/api/push-endpoints",
        &token,
        Some((HERE, false)),
        &registering("pixel", "http://push.example/abc"),
    )
    .await;
    assert_eq!(code(&plain), "PUSH-3", "{}", plain.body);

    let (stray, _) = sent(
        router,
        "POST",
        "/api/push-endpoints",
        &token,
        Some((HERE, false)),
        r#"{"name":"pixel"}"#,
    )
    .await;
    assert_eq!(code(&stray), "ASK-11", "{}", stray.body);
}

/// A registration carries a secret, so it does not cross a network in the clear.
#[tokio::test]
async fn a_registration_does_not_cross_a_network_in_the_clear() {
    let (router, token) = a_machine("clear");
    let (crossed, _) = sent(
        router,
        "POST",
        "/api/push-endpoints",
        &token,
        Some((NEXT_DOOR, false)),
        &registering("pixel", "https://push.example/abc"),
    )
    .await;
    assert!(crossed.status.is_client_error(), "{}", crossed.body);
    assert!(!code(&crossed).starts_with("PUSH"), "{}", crossed.body);
}
