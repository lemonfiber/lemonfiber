//! What a device key proves: the operator, for one mint, signed over a challenge the
//! core issued to the token that asked for it.

use base64::Engine as _;
use lemonfiber_core::device_keys::proof::Mint;
use lemonfiber_fixtures::attestations::ANDROID_RSA_ROOT;
use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, KeyPair as _, ECDSA_P256_SHA256_ASN1_SIGNING};

use lemonfiber_api::admission::Keyring;
use lemonfiber_core::keys::{Kept, Minter, Purpose, Record, Scope};

use crate::door::*;
use crate::what_a_key_admits::{a_secret, code, presented, sent, HERE, NEXT_DOOR};

/// What a P-256 `SubjectPublicKeyInfo` holds ahead of its point.
const SPKI_PREFIX: [u8; 26] = [
    0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a,
    0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
];

/// `bytes` in base64url, unpadded.
fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// A phone's key pair, as its hardware would hold one.
fn a_phone() -> EcdsaKeyPair {
    let random = SystemRandom::new();
    let made = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &random)
        .ok()
        .and_then(|pkcs8| {
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &random).ok()
        });
    made.unwrap_or_else(|| unreachable!("this machine makes a key pair"))
}

/// The public half of `phone`, as its `SubjectPublicKeyInfo`.
fn public_half(phone: &EcdsaKeyPair) -> Vec<u8> {
    [SPKI_PREFIX.as_slice(), phone.public_key().as_ref()].concat()
}

/// `phone`'s signature over `text`, in base64url.
fn signature(phone: &EcdsaKeyPair, text: &str) -> String {
    phone
        .sign(&SystemRandom::new(), text.as_bytes())
        .map(|signed| b64(signed.as_ref()))
        .unwrap_or_default()
}

/// A machine with a password kept, its clock where the recorded chain is valid, and
/// `phone` registered as `pixel` where one is given.
fn a_machine(named: &str, phone: Option<&EcdsaKeyPair>) -> (axum::Router, String, PathBuf) {
    let (router, token, env, _) = a_machine_keeping(named, phone);
    (router, token, env)
}

/// The same machine, and where it keeps its password.
fn a_machine_keeping(
    named: &str,
    phone: Option<&EcdsaKeyPair>,
) -> (axum::Router, String, PathBuf, PathBuf) {
    built(named, phone, not_the_token(), Keyring::default())
}

/// The same machine, its randomness drawn from `random` and its keys from `keys`.
fn built(
    named: &str,
    phone: Option<&EcdsaKeyPair>,
    random: Chance,
    keys: Keyring,
) -> (axum::Router, String, PathBuf, PathBuf) {
    let admission = keeping(&format!("device-{named}"));
    let mut ctx = world(Some(admission.clone()), random);
    ctx.seams.clock = Stopped::at(ANDROID_RSA_ROOT.at);
    let env = a_directory(&format!("device-env-{named}")).join(".env");
    if let Some(dir) = env.parent() {
        let _ = fs::create_dir_all(dir);
    }
    ctx.settings.env_file = Some(env.clone());
    ctx.settings.stack_dir = env.parent().map(|dir| dir.join("stack"));
    if let Some(phone) = phone {
        let under = credential::at(&admission)
            .map(|held| held.fingerprint())
            .unwrap_or_default();
        let app = format!("android:app.lemonfiber.companion@{}", "5a".repeat(32));
        let settings = serde_json::json!({
            "schema": 1,
            "written_by": "0.0.0",
            "values": { lemonfiber_core::device_keys::identity::SETTING: app },
        });
        let _ = fs::write(env.with_file_name("settings.json"), settings.to_string());
        let record = serde_json::json!({
            "schema": 1,
            "written_by": "0.0.0",
            "keys": [{
                "name": "pixel",
                "public_key": b64(&public_half(phone)),
                "fingerprint": "f".repeat(64),
                "backing": "strongbox",
                "app": app,
                "registered": "2025-01-08T00:00:00Z",
                "under": under,
            }],
        });
        let _ = fs::write(
            env.with_file_name(lemonfiber_core::device_keys::FILE),
            record.to_string(),
        );
    }
    let admitting = Arc::new(Admitting {
        kept: Some(admission.clone()),
        keys,
        ..Admitting::default()
    });
    let (router, token) = surface(ctx, &admitting);
    (router, token.as_str().to_owned(), env, admission)
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

/// A challenge for `pixel`, asked with `token`.
async fn challenge(router: &axum::Router, token: &str) -> String {
    let answer = presented(
        router.clone(),
        "POST",
        "/api/device-keys/pixel/challenge",
        token,
        Some((HERE, false)),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(read(&answer.body, "/kind"), "device-challenge");
    read(&answer.body, "/data/challenge")
}

/// The mint of a `read` key named `name`, proved by `pixel` over `challenge` with
/// `signature`.
fn proved_mint(name: &str, challenge: &str, signature: &str) -> String {
    serde_json::json!({
        "name": name,
        "scope": "read",
        "purpose": "home-assistant",
        "device": "pixel",
        "challenge": challenge,
        "signature": signature,
    })
    .to_string()
}

/// What `phone` signs to mint a `read` key named `name` under `challenge`.
fn signed_for(phone: &EcdsaKeyPair, name: &str, challenge: &str) -> String {
    let mint = Mint {
        name,
        scope: "read",
        purpose: "home-assistant",
        claim: false,
    };
    signature(phone, &mint.signed(challenge))
}

/// Ask to mint with `body`, as the holder of `token`.
async fn minting(router: &axum::Router, token: &str, body: &str) -> Answer {
    sent(
        router.clone(),
        "POST",
        "/api/keys",
        token,
        Some((HERE, false)),
        body,
    )
    .await
    .0
}

#[tokio::test]
async fn a_signature_over_an_issued_challenge_mints_in_place_of_the_password_once() {
    let phone = a_phone();
    let (router, token, env) = a_machine("mints", Some(&phone));
    let issued = challenge(&router, &token).await;
    let body = proved_mint("ha", &issued, &signed_for(&phone, "ha", &issued));
    let minted = minting(&router, &token, &body).await;
    assert_eq!(minted.status, StatusCode::OK, "{}", minted.body);
    assert_eq!(read(&minted.body, "/kind"), "minted-key");
    let kept = fs::read_to_string(env.with_file_name(lemonfiber_core::device_keys::FILE))
        .unwrap_or_default();
    assert!(kept.contains("\"used\": \"2025-01-08"), "{kept}");
    let again = minting(&router, &token, &body.replace("\"ha\"", "\"ha2\"")).await;
    assert_eq!(code(&again), "DEVICE-10");
    let listed = presented(router, "GET", "/api/keys", &token, Some((HERE, false))).await;
    assert_eq!(
        read(&listed.body, "/data/keys/0/proved_by"),
        "pixel",
        "{}",
        listed.body
    );
}

#[tokio::test]
async fn a_signature_over_another_mint_or_by_another_key_proves_nothing_and_is_heard() {
    let phone = a_phone();
    let (router, token, env) = a_machine("misproved", Some(&phone));
    let issued = challenge(&router, &token).await;
    let body = proved_mint("ha", &issued, &signed_for(&phone, "another", &issued));
    let refused = minting(&router, &token, &body).await;
    assert_eq!(refused.status, StatusCode::UNAUTHORIZED);
    assert_eq!(code(&refused), "DEVICE-11");
    let issued = challenge(&router, &token).await;
    let body = proved_mint("ha", &issued, &signed_for(&a_phone(), "ha", &issued));
    assert_eq!(code(&minting(&router, &token, &body).await), "DEVICE-11");
    assert!(!env.with_file_name("keys.json").exists());
}

#[tokio::test]
async fn wrong_signatures_meet_the_limit_wrong_passwords_meet() {
    let phone = a_phone();
    let (router, token, _) = a_machine("limited", Some(&phone));
    let mut last = None;
    for _ in 0..6 {
        let issued = challenge(&router, &token).await;
        let body = proved_mint("ha", &issued, &signed_for(&phone, "other", &issued));
        last = Some(minting(&router, &token, &body).await);
    }
    assert_eq!(
        last.map(|answer| answer.status),
        Some(StatusCode::TOO_MANY_REQUESTS)
    );
}

#[tokio::test]
async fn a_challenge_never_issued_or_issued_to_another_token_proves_nothing() {
    let phone = a_phone();
    let (router, token, _) = a_machine("unissued", Some(&phone));
    let made_up = b64(&[7; 32]);
    let body = proved_mint("ha", &made_up, &signed_for(&phone, "ha", &made_up));
    assert_eq!(code(&minting(&router, &token, &body).await), "DEVICE-10");
    let opened = offered_from(router.clone(), HERE, &offering(&chosen())).await;
    let other = session(&opened.body);
    let issued = challenge(&router, &other).await;
    let body = proved_mint("ha", &issued, &signed_for(&phone, "ha", &issued));
    let refused = minting(&router, &token, &body).await;
    assert_eq!(
        code(&refused),
        "DEVICE-10",
        "{} {}",
        refused.status,
        refused.body
    );
}

#[tokio::test]
async fn a_device_key_not_registered_is_refused_before_any_challenge_is_looked_at() {
    let (router, token, _) = a_machine("unregistered", None);
    let refused = presented(
        router.clone(),
        "POST",
        "/api/device-keys/pixel/challenge",
        &token,
        Some((HERE, false)),
    )
    .await;
    assert_eq!(code(&refused), "DEVICE-9");
    let body = proved_mint("ha", "anything", "anything");
    assert_eq!(code(&minting(&router, &token, &body).await), "DEVICE-9");
}

#[tokio::test]
async fn a_challenge_issued_before_its_device_key_was_revoked_proves_nothing() {
    let phone = a_phone();
    let (router, token, _) = a_machine("revoked-between", Some(&phone));
    let issued = challenge(&router, &token).await;
    let revoked = presented(
        router.clone(),
        "DELETE",
        "/api/device-keys/pixel",
        &token,
        Some((HERE, false)),
    )
    .await;
    assert_eq!(revoked.status, StatusCode::OK, "{}", revoked.body);
    let body = proved_mint("ha", &issued, &signed_for(&phone, "ha", &issued));
    assert_eq!(code(&minting(&router, &token, &body).await), "DEVICE-9");
}

#[tokio::test]
async fn a_password_changed_elsewhere_cuts_off_a_challenge_already_issued() {
    let phone = a_phone();
    let (router, token, env, admission) = a_machine_keeping("password-between", Some(&phone));
    let issued = challenge(&router, &token).await;
    let changed = credential::keep(
        &admission,
        &a_credential(&lemonfiber_fixtures::passwords::another_password()),
    );
    assert!(changed.is_ok());
    let body = proved_mint("ha", &issued, &signed_for(&phone, "ha", &issued));
    assert_eq!(code(&minting(&router, &token, &body).await), "DEVICE-9");
    let kept = fs::read_to_string(env.with_file_name(lemonfiber_core::device_keys::FILE))
        .unwrap_or_default();
    assert!(kept.contains("\"revoked\""), "{kept}");
}

#[tokio::test]
async fn a_mint_carrying_both_proofs_or_half_of_one_is_not_a_mint() {
    let phone = a_phone();
    let (router, token, _) = a_machine("halves", Some(&phone));
    let both = serde_json::json!({
        "name": "ha", "scope": "read", "purpose": "home-assistant",
        "password": chosen(), "device": "pixel", "challenge": "c", "signature": "s",
    })
    .to_string();
    assert_eq!(code(&minting(&router, &token, &both).await), "ASK-11");
    let half = serde_json::json!({
        "name": "ha", "scope": "read", "purpose": "home-assistant", "device": "pixel",
    })
    .to_string();
    assert_eq!(code(&minting(&router, &token, &half).await), "ASK-11");
}

/// The recorded phone's registration, asked with `password`.
fn registering(password: &str) -> String {
    let chain: Vec<String> = ANDROID_RSA_ROOT.chain.iter().map(|der| b64(der)).collect();
    let own = ANDROID_RSA_ROOT.chain.first().copied().unwrap_or_default();
    serde_json::json!({
        "name": "pixel",
        "public_key": b64(spki_of(own)),
        "attestation": { "platform": "android", "chain": chain },
        "password": password,
    })
    .to_string()
}

/// The `SubjectPublicKeyInfo` a DER certificate carries: its first sequence whose
/// algorithm is an elliptic-curve key.
fn spki_of(certificate: &[u8]) -> &[u8] {
    let marker = [
        0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01,
    ];
    certificate
        .windows(marker.len())
        .position(|window| window == marker)
        .and_then(|at| certificate.get(at..at + 91))
        .unwrap_or_default()
}

#[tokio::test]
async fn a_registration_asks_for_the_password_and_answers_with_what_the_attestation_vouched_for() {
    let (router, token, env) = a_machine("registers", None);
    let (wrong, _) = sent(
        router.clone(),
        "POST",
        "/api/device-keys",
        &token,
        Some((HERE, false)),
        &registering(&lemonfiber_fixtures::passwords::another_password()),
    )
    .await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED, "{}", wrong.body);
    let (untrusted, _) = sent(
        router.clone(),
        "POST",
        "/api/device-keys",
        &token,
        Some((HERE, false)),
        &registering(&chosen()),
    )
    .await;
    assert_eq!(code(&untrusted), "DEVICE-8", "{}", untrusted.body);
    let named = read(&untrusted.body, "/data/summary");
    let identity = named
        .strip_prefix("This stack does not trust ")
        .unwrap_or_default()
        .to_owned();
    let settings = serde_json::json!({
        "schema": 1, "written_by": "0.0.0",
        "values": { "LEMONFIBER_DEVICE_KEY_APPS": identity },
    });
    let _ = fs::write(env.with_file_name("settings.json"), settings.to_string());
    let (registered, _) = sent(
        router.clone(),
        "POST",
        "/api/device-keys",
        &token,
        Some((HERE, false)),
        &registering(&chosen()),
    )
    .await;
    assert_eq!(registered.status, StatusCode::OK, "{}", registered.body);
    assert_eq!(read(&registered.body, "/kind"), "device-key");
    assert_eq!(read(&registered.body, "/data/backing"), "tee");
    let listed = presented(
        router.clone(),
        "GET",
        "/api/device-keys",
        &token,
        Some((HERE, false)),
    )
    .await;
    assert_eq!(read(&listed.body, "/kind"), "device-keys");
    assert_eq!(read(&listed.body, "/data/keys/0/name"), "pixel");
    let revoked = presented(
        router,
        "DELETE",
        "/api/device-keys/pixel",
        &token,
        Some((HERE, false)),
    )
    .await;
    assert_eq!(read(&revoked.body, "/data/revoked"), "pixel");
}

#[tokio::test]
async fn a_registration_from_another_machine_in_the_clear_is_refused_before_the_password() {
    let (router, token, _) = a_machine("clear", None);
    let (refused, _) = sent(
        router,
        "POST",
        "/api/device-keys",
        &token,
        Some((NEXT_DOOR, false)),
        &registering(&chosen()),
    )
    .await;
    assert_eq!(code(&refused), "ADMIT-11", "{}", refused.body);
}

#[tokio::test]
async fn a_key_is_refused_at_every_device_key_route_whatever_its_scope() {
    let secret = a_secret(7);
    let dir = a_directory("device-keyed");
    let _ = fs::create_dir_all(&dir);
    let kept = Kept {
        keys: vec![Record::minted(
            "actor",
            Scope::Act,
            Purpose::Other,
            &secret,
            "2026-10-05T06:00:00".to_owned(),
            Minter::Operator,
        )],
    };
    assert!(kept.keep(&dir.join("keys.json")).is_ok());
    let keys = Keyring::at(
        Some(dir.join("keys.json")),
        Some(dir.join("keys-used.json")),
    );
    let (router, _, _, _) = built("keyed", Some(&a_phone()), not_the_token(), keys);
    for (method, path) in [
        ("GET", "/api/device-keys"),
        ("POST", "/api/device-keys"),
        ("DELETE", "/api/device-keys/pixel"),
        ("POST", "/api/device-keys/pixel/challenge"),
    ] {
        let answer = presented(
            router.clone(),
            method,
            path,
            secret.as_str(),
            Some((HERE, false)),
        )
        .await;
        assert_eq!(
            answer.status,
            StatusCode::FORBIDDEN,
            "{method} {path}: {}",
            answer.body
        );
    }
}

#[tokio::test]
async fn a_registration_that_is_not_one_is_refused_and_guessing_its_password_earns_the_wait() {
    let (router, token, _) = a_machine("misregistered", None);
    let (unread, _) = sent(
        router.clone(),
        "POST",
        "/api/device-keys",
        &token,
        Some((HERE, false)),
        r#"{"name": "pixel"}"#,
    )
    .await;
    assert_eq!(unread.status, StatusCode::BAD_REQUEST, "{}", unread.body);
    let mut held = false;
    for _ in 0..40 {
        let (answer, _) = sent(
            router.clone(),
            "POST",
            "/api/device-keys",
            &token,
            Some((HERE, false)),
            &registering(&lemonfiber_fixtures::passwords::another_password()),
        )
        .await;
        if answer.status == StatusCode::TOO_MANY_REQUESTS {
            held = true;
            break;
        }
    }
    assert!(held);
}

#[tokio::test]
async fn a_machine_that_will_not_supply_randomness_issues_no_challenge() {
    let phone = a_phone();
    let (router, token, _, _) = built(
        "unrandom",
        Some(&phone),
        Chance::exactly(None),
        Keyring::default(),
    );
    let answer = presented(
        router,
        "POST",
        "/api/device-keys/pixel/challenge",
        &token,
        Some((HERE, false)),
    )
    .await;
    assert_eq!(code(&answer), "DEVICE-12", "{}", answer.body);
}

#[tokio::test]
async fn a_mint_carrying_no_proof_is_refused_as_no_key_request() {
    let (router, token, _) = a_machine("unproved", None);
    let body = serde_json::json!({ "name": "ha", "scope": "read", "purpose": "other" }).to_string();
    let answer = minting(&router, &token, &body).await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{}", answer.body);
}
