//! What a mint takes, from the operator and from a household member.
//!
//! The password again in the same request, a connection that may carry the secret back,
//! and, for a member, the operator's leave and a key scoped to them alone.

use std::path::PathBuf;

use lemonfiber_api::admission::Keyring;

use crate::door::*;
use crate::what_a_key_admits::{code, presented, sent, HERE, NEXT_DOOR};
use lemonfiber_fixtures::http::Answer as Reply;

/// A machine with a password kept and nothing minted, served by its own token.
fn minting_machine(named: &str) -> (axum::Router, String, PathBuf) {
    let admission = keeping(&format!("mint-{named}"));
    let mut ctx = world(Some(admission.clone()), not_the_token());
    let env = a_directory(&format!("mint-env-{named}")).join(".env");
    if let Some(dir) = env.parent() {
        let _ = fs::create_dir_all(dir);
    }
    ctx.settings.env_file = Some(env.clone());
    ctx.settings.stack_dir = env.parent().map(|dir| dir.join("stack"));
    let admitting = Arc::new(Admitting {
        kept: Some(admission),
        keys: Keyring::at(
            lemonfiber_core::keys::run::at(&ctx),
            lemonfiber_core::keys::run::used_at(&ctx),
        ),
        ..Admitting::default()
    });
    let (router, token) = surface(ctx, &admitting);
    (router, token.as_str().to_owned(), env)
}

/// What a mint is asked with, under `password`.
fn mint_body(name: &str, password: &str) -> String {
    serde_json::json!({
        "name": name,
        "scope": "act",
        "purpose": "home-assistant",
        "password": password,
    })
    .to_string()
}

/// The secret a mint reply hands back.
fn handed(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|reply| {
            reply
                .pointer("/data/secret")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn the_operator_mints_with_the_password_and_sees_the_secret_once() {
    let (router, token, env) = minting_machine("minted");
    let (minted, headers) = sent(
        router.clone(),
        "POST",
        "/api/keys",
        &token,
        Some((HERE, false)),
        &mint_body("ha", &chosen()),
    )
    .await;
    assert_eq!(minted.status, StatusCode::OK, "{}", minted.body);
    assert_eq!(
        headers
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    let secret = handed(&minted.body);
    assert!(lemonfiber_core::keys::shaped(&secret), "{}", minted.body);
    let listed = presented(
        router.clone(),
        "GET",
        "/api/keys",
        &token,
        Some((HERE, false)),
    )
    .await;
    assert!(listed.body.contains("\"ha\""), "{}", listed.body);
    assert!(!listed.body.contains(&secret));
    let kept = fs::read_to_string(env.with_file_name("keys.json")).unwrap_or_default();
    assert!(kept.contains("\"ha\"") && !kept.contains(&secret));
    let used = presented(
        router.clone(),
        "GET",
        "/api/explain?word=indexer",
        &secret,
        Some((HERE, false)),
    )
    .await;
    assert_eq!(used.status, StatusCode::OK, "{}", used.body);
    let revoked = presented(
        router.clone(),
        "DELETE",
        "/api/keys/ha",
        &token,
        Some((HERE, false)),
    )
    .await;
    assert_eq!(revoked.status, StatusCode::OK, "{}", revoked.body);
    let after = presented(
        router,
        "GET",
        "/api/explain?word=indexer",
        &secret,
        Some((HERE, false)),
    )
    .await;
    assert_eq!(code(&after), "ADMIT-4");
}

#[tokio::test]
async fn a_mint_without_the_right_password_mints_nothing() {
    let (router, token, env) = minting_machine("unproven");
    let (refused, _) = sent(
        router,
        "POST",
        "/api/keys",
        &token,
        Some((HERE, false)),
        &mint_body("ha", &nobodys()),
    )
    .await;
    assert_eq!(refused.status, StatusCode::UNAUTHORIZED, "{}", refused.body);
    assert!(!env.with_file_name("keys.json").exists());
}

#[tokio::test]
async fn a_mint_asked_in_anything_but_its_own_shape_is_refused() {
    let (router, token, env) = minting_machine("misshapen");
    let (refused, _) = sent(
        router,
        "POST",
        "/api/keys",
        &token,
        Some((HERE, false)),
        "name=ha&scope=act",
    )
    .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{}", refused.body);
    assert_eq!(code(&refused), "ASK-11");
    assert!(!env.with_file_name("keys.json").exists());
}

#[tokio::test]
async fn a_mint_from_another_machine_in_the_clear_is_refused_before_the_password_is_tried() {
    let (router, token, env) = minting_machine("in-the-clear");
    for from in [Some((NEXT_DOOR, false)), None] {
        let (refused, _) = sent(
            router.clone(),
            "POST",
            "/api/keys",
            &token,
            from,
            &mint_body("ha", &chosen()),
        )
        .await;
        assert_eq!(code(&refused), "ADMIT-11", "{}", refused.body);
    }
    assert!(!env.with_file_name("keys.json").exists());
}

/// What the media server answers a name and password it recognises with.
const SIGNED_IN: &str = r#"{"AccessToken":"a-session","User":{"Id":"a7f3"}}"#;

/// What it answers when asked whether that member's account still stands.
const STANDING: &str = r#"{"Id":"a7f3","HasPassword":true}"#;

/// The household it holds: Ana, and nobody else.
const HOUSEHOLD: &str = r#"[{"Id":"a7f3","Name":"ana","HasPassword":true,
    "Policy":{"IsAdministrator":false,"EnableAllFolders":true}}]"#;

/// A machine over the stack's own media server, where Ana holds a session, the operator
/// having allowed members to mint keys or not.
///
/// The media server answers the passwords it is given in `signing`, in turn: the first
/// is Ana's sign-in, and each later one a password a mint gave again.
async fn a_member_signed_in(
    named: &str,
    allowed: bool,
    signing: Vec<Reply>,
) -> (axum::Router, String, PathBuf, Arc<Fake>) {
    holding(named, allowed, signing, HOUSEHOLD).await
}

/// The same, over a media server whose list of its household is `household`.
async fn holding(
    named: &str,
    allowed: bool,
    signing: Vec<Reply>,
    household: &'static str,
) -> (axum::Router, String, PathBuf, Arc<Fake>) {
    let transport = Fake::by_path_in_turn(vec![
        ("/Users/AuthenticateByName", signing),
        ("/Users/Me", vec![Reply::reply(200, STANDING)]),
        ("/Users", vec![Reply::reply(200, household)]),
    ]);
    let mut ctx = a_stack(&format!("member-mint-{named}"), transport.clone());
    seeded(&ctx);
    let Some(env) = ctx.settings.env_file.clone() else {
        unreachable!("a stack built here keeps its configuration somewhere")
    };
    if allowed {
        assert!(lemonfiber_core::config::store::set(
            &env,
            lemonfiber_core::config::MEMBER_KEYS_KEY,
            "on"
        )
        .is_ok());
    }
    ctx.settings.stack_dir = env.parent().map(|dir| dir.join("stack"));
    let admitting = Arc::new(Admitting {
        keys: Keyring::at(
            lemonfiber_core::keys::run::at(&ctx),
            lemonfiber_core::keys::run::used_at(&ctx),
        ),
        household: Some(Arc::new(ctx.clone()) as Arc<dyn HouseholdAtHand>),
        ..Admitting::default()
    });
    let (router, _) = surface(ctx, &admitting);
    let opened = asked(
        router.clone(),
        "POST",
        SESSION,
        &from_here(),
        &offering_as("ana", &hers()),
    )
    .await;
    assert_eq!(opened.status, StatusCode::OK, "{}", opened.body);
    (router, session(&opened.body), env, transport)
}

/// How many times the media server was asked to sign somebody in.
fn sign_ins(transport: &Fake) -> usize {
    transport
        .requests()
        .iter()
        .filter(|request| request.url.ends_with("/Users/AuthenticateByName"))
        .count()
}

/// What a member's mint is asked with, under `scope` and `password`.
fn member_mint(name: &str, scope: &str, password: &str) -> String {
    serde_json::json!({
        "name": name,
        "scope": scope,
        "purpose": "mcp",
        "password": password,
    })
    .to_string()
}

#[tokio::test]
async fn a_member_mints_nothing_until_the_operator_allows_it() {
    let (router, member, env, transport) =
        a_member_signed_in("unallowed", false, vec![Reply::reply(200, SIGNED_IN)]).await;
    let (refused, _) = sent(
        router,
        "POST",
        "/api/keys",
        &member,
        Some((HERE, false)),
        &member_mint("anas", "member:ana", &hers()),
    )
    .await;
    assert_eq!(code(&refused), "KEY-11", "{}", refused.body);
    assert!(!env.with_file_name("keys.json").exists());
    // Refused before the password was put to the media server: only the sign-in was.
    assert_eq!(sign_ins(&transport), 1);
}

#[tokio::test]
async fn an_allowed_member_mints_a_key_for_themselves_with_their_password() {
    let (router, member, _, _) = a_member_signed_in(
        "allowed",
        true,
        vec![
            Reply::reply(200, SIGNED_IN),
            Reply::reply(401, ""),
            Reply::reply(200, SIGNED_IN),
        ],
    )
    .await;
    let (wrong, _) = sent(
        router.clone(),
        "POST",
        "/api/keys",
        &member,
        Some((HERE, false)),
        &member_mint("anas", "member:ana", &nobodys()),
    )
    .await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED, "{}", wrong.body);
    let (other, _) = sent(
        router.clone(),
        "POST",
        "/api/keys",
        &member,
        Some((HERE, false)),
        &member_mint("anas", "act", &hers()),
    )
    .await;
    assert_eq!(code(&other), "KEY-8", "{}", other.body);
    let (minted, headers) = sent(
        router.clone(),
        "POST",
        "/api/keys",
        &member,
        Some((HERE, false)),
        &member_mint("anas", "member:ana", &hers()),
    )
    .await;
    assert_eq!(minted.status, StatusCode::OK, "{}", minted.body);
    assert_eq!(
        headers
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    let secret = handed(&minted.body);
    let listed = presented(
        router.clone(),
        "GET",
        "/api/keys",
        &member,
        Some((HERE, false)),
    )
    .await;
    assert!(listed.body.contains("\"anas\""), "{}", listed.body);
    assert!(!listed.body.contains(&secret));
    let used = presented(
        router.clone(),
        "GET",
        "/api/requests",
        &secret,
        Some((HERE, false)),
    )
    .await;
    assert_ne!(used.status, StatusCode::FORBIDDEN, "{}", used.body);
    let revoked = presented(
        router.clone(),
        "DELETE",
        "/api/keys/anas",
        &member,
        Some((HERE, false)),
    )
    .await;
    assert_eq!(revoked.status, StatusCode::OK, "{}", revoked.body);
    let after = presented(router, "GET", "/api/requests", &secret, Some((HERE, false))).await;
    assert_eq!(code(&after), "ADMIT-4");
}

#[tokio::test]
async fn a_member_mint_from_another_machine_in_the_clear_is_refused() {
    let (router, member, env, _) =
        a_member_signed_in("in-the-clear", true, vec![Reply::reply(200, SIGNED_IN)]).await;
    let (refused, _) = sent(
        router,
        "POST",
        "/api/keys",
        &member,
        Some((NEXT_DOOR, false)),
        &member_mint("anas", "member:ana", &hers()),
    )
    .await;
    assert_eq!(code(&refused), "ADMIT-11", "{}", refused.body);
    assert!(!env.with_file_name("keys.json").exists());
}

#[tokio::test]
async fn a_password_that_proves_another_account_mints_nothing() {
    let somebody_else = r#"{"AccessToken":"a-session","User":{"Id":"b8c4"}}"#;
    let (router, member, env, _) = a_member_signed_in(
        "another-account",
        true,
        vec![
            Reply::reply(200, SIGNED_IN),
            Reply::reply(200, somebody_else),
        ],
    )
    .await;
    let (refused, _) = sent(
        router,
        "POST",
        "/api/keys",
        &member,
        Some((HERE, false)),
        &member_mint("anas", "member:ana", &hers()),
    )
    .await;
    assert_eq!(refused.status, StatusCode::UNAUTHORIZED, "{}", refused.body);
    assert!(!env.with_file_name("keys.json").exists());
}

#[tokio::test]
async fn a_machine_keeping_no_password_mints_nothing_over_the_web() {
    let mut ctx = world(None, not_the_token());
    let env = a_directory("mint-env-no-password").join(".env");
    if let Some(dir) = env.parent() {
        let _ = fs::create_dir_all(dir);
    }
    ctx.settings.env_file = Some(env.clone());
    ctx.settings.stack_dir = env.parent().map(|dir| dir.join("stack"));
    let admitting = Arc::new(Admitting {
        keys: Keyring::at(
            lemonfiber_core::keys::run::at(&ctx),
            lemonfiber_core::keys::run::used_at(&ctx),
        ),
        ..Admitting::default()
    });
    let (router, token) = surface(ctx, &admitting);
    let (refused, _) = sent(
        router,
        "POST",
        "/api/keys",
        token.as_str(),
        Some((HERE, false)),
        &mint_body("ha", &chosen()),
    )
    .await;
    assert_eq!(refused.status, StatusCode::UNAUTHORIZED, "{}", refused.body);
    assert!(!env.with_file_name("keys.json").exists());
}

#[tokio::test]
async fn wrong_passwords_at_a_mint_earn_the_wait_a_sign_in_does() {
    let (router, token, env) = minting_machine("guessed");
    let mut held = None;
    for _ in 0..8 {
        let (answer, _) = sent(
            router.clone(),
            "POST",
            "/api/keys",
            &token,
            Some((NEXT_DOOR, true)),
            &mint_body("ha", &nobodys()),
        )
        .await;
        if answer.status == StatusCode::TOO_MANY_REQUESTS {
            held = Some(answer);
            break;
        }
    }
    assert!(held.is_some_and(|held| held.left.is_some()));
    assert!(!env.with_file_name("keys.json").exists());
}

#[tokio::test]
async fn a_member_the_household_no_longer_lists_mints_nothing() {
    let (router, member, env, _) =
        holding("unlisted", true, vec![Reply::reply(200, SIGNED_IN)], "[]").await;
    let (refused, _) = sent(
        router,
        "POST",
        "/api/keys",
        &member,
        Some((HERE, false)),
        &member_mint("anas", "member:ana", &hers()),
    )
    .await;
    assert_eq!(refused.status, StatusCode::UNAUTHORIZED, "{}", refused.body);
    assert!(!env.with_file_name("keys.json").exists());
}
