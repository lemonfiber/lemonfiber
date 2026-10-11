//! What a key minted as a claim is collected with, and what a claim cannot collect.
//!
//! The program holds nothing but the claim, so the door is open to it as the sign-in
//! door is, and guarded the same way: only over a connection a secret may cross, and
//! counted as a guess at a key until it collects one.

use std::path::PathBuf;

use lemonfiber_api::admission::Keyring;
use lemonfiber_core::keys::{Claim, Kept, Minter, Purpose, Record, Scope};

use crate::door::*;
use crate::what_a_key_admits::{code, presented, sent, NEXT_DOOR};

/// A machine keeping one key waiting on `claim`, its lapse far ahead of the stopped
/// clock, and nothing served on the network.
fn waiting_machine(named: &str, claim: &Claim) -> (axum::Router, String, PathBuf) {
    let admission = keeping(&format!("claim-{named}"));
    let mut ctx = world(Some(admission.clone()), not_the_token());
    let env = a_directory(&format!("claim-env-{named}")).join(".env");
    if let Some(dir) = env.parent() {
        let _ = fs::create_dir_all(dir);
    }
    ctx.settings.env_file = Some(env.clone());
    ctx.settings.stack_dir = env.parent().map(|dir| dir.join("stack"));
    let kept_at = lemonfiber_core::keys::run::at(&ctx).unwrap_or_default();
    let waiting = Record::claimed(
        "ha",
        Scope::Read,
        Purpose::HomeAssistant,
        claim,
        "2099-01-01T00:00:00".to_owned(),
        "2026-10-05T06:00:00".to_owned(),
        Minter::Operator,
    );
    assert!(Kept {
        keys: vec![waiting]
    }
    .keep(&kept_at)
    .is_ok());
    let admitting = Arc::new(Admitting {
        kept: Some(admission),
        keys: Keyring::at(Some(kept_at), lemonfiber_core::keys::run::used_at(&ctx)),
        ..Admitting::default()
    });
    let (router, token) = surface(ctx, &admitting);
    (router, token.as_str().to_owned(), env)
}

/// A claim minted from a source answering with `byte`.
fn a_claim(byte: u8) -> Claim {
    let Some(claim) = Claim::mint(&Chance::exactly(Some(vec![byte; 32]))) else {
        unreachable!("thirty-two bytes mint a claim")
    };
    claim
}

fn collecting(claim: &str) -> String {
    serde_json::json!({ "claim": claim }).to_string()
}

#[tokio::test]
async fn a_claim_collects_its_key_once_over_tls_and_the_key_is_let_in() {
    let claim = a_claim(7);
    let (router, _, _) = waiting_machine("collects", &claim);
    let (collected, headers) = sent(
        router.clone(),
        "POST",
        "/api/keys/claim",
        "",
        Some((NEXT_DOOR, true)),
        &collecting(claim.as_str()),
    )
    .await;
    assert_eq!(collected.status, StatusCode::OK, "{}", collected.body);
    assert_eq!(
        headers
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    let secret = serde_json::from_str::<serde_json::Value>(&collected.body)
        .ok()
        .and_then(|reply| {
            reply
                .pointer("/data/secret")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default();
    assert!(lemonfiber_core::keys::shaped(&secret), "{}", collected.body);
    let reading = presented(
        router.clone(),
        "GET",
        "/api/explain?word=indexer",
        &secret,
        Some((NEXT_DOOR, true)),
    )
    .await;
    assert_eq!(reading.status, StatusCode::OK, "{}", reading.body);
    let (again, _) = sent(
        router,
        "POST",
        "/api/keys/claim",
        "",
        Some((NEXT_DOOR, true)),
        &collecting(claim.as_str()),
    )
    .await;
    assert_eq!(again.status, StatusCode::UNAUTHORIZED, "{}", again.body);
    assert_eq!(code(&again), "KEY-12");
}

#[tokio::test]
async fn a_claim_sent_in_the_clear_from_another_machine_collects_nothing() {
    let claim = a_claim(8);
    let (router, _, _) = waiting_machine("clear", &claim);
    let (refused, _) = sent(
        router,
        "POST",
        "/api/keys/claim",
        "",
        Some((NEXT_DOOR, false)),
        &collecting(claim.as_str()),
    )
    .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{}", refused.body);
}

#[tokio::test]
async fn guessed_claims_earn_the_wait_guessed_keys_do() {
    let (router, _, _) = waiting_machine("guessed", &a_claim(9));
    let mut held = false;
    for byte in 10..40u8 {
        let (answer, _) = sent(
            router.clone(),
            "POST",
            "/api/keys/claim",
            "",
            Some((NEXT_DOOR, true)),
            &collecting(a_claim(byte).as_str()),
        )
        .await;
        if answer.status == StatusCode::TOO_MANY_REQUESTS {
            held = true;
            break;
        }
        assert_eq!(answer.status, StatusCode::UNAUTHORIZED, "{}", answer.body);
    }
    assert!(held);
}

#[tokio::test]
async fn a_claim_is_not_minted_where_no_other_machine_reaches_the_stack() {
    let (router, token, _) = waiting_machine("unlinkable", &a_claim(11));
    let body = serde_json::json!({
        "name": "mcp",
        "scope": "read",
        "purpose": "mcp",
        "password": chosen(),
        "claim": true,
    })
    .to_string();
    let (refused, _) = sent(
        router,
        "POST",
        "/api/keys",
        &token,
        Some((std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), false)),
        &body,
    )
    .await;
    assert_eq!(refused.status, StatusCode::CONFLICT, "{}", refused.body);
    assert_eq!(code(&refused), "KEY-13");
}
