//! The key lemonfiber keeps on the media server, and choosing an identity.

use super::{reader, OURS_TOO, SIGNED_IN, SOMEONE_ELSES};
use lemonfiber_core::ports::http::Method;
use lemonfiber_core::ports::service::{Allowed, Failure};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::Household;

/// A key already filed under lemonfiber's name is handed back, not minted again.
///
/// Seeding runs repeatedly, and a fresh key each time would leave every earlier one
/// valid on the server for as long as the stack lives.
#[tokio::test]
async fn a_key_already_ours_is_reused_rather_than_minted_again() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, OURS_TOO),
    ]);
    let held = reader(&fake).api_key().await;

    // The value is not in the message: it is a credential, and a failing
    // assertion prints its message into the run's log.
    assert!(
        held.is_ok_and(|key| key == "ours"),
        "the key already filed under our name was not handed back"
    );
    assert!(
        !fake
            .requests()
            .iter()
            .any(|asked| asked.method == Method::Post && asked.url.contains("/Auth/Keys")),
        "a key that already existed was minted a second time"
    );
}

/// With no key of ours, one is minted and then read back from the list.
///
/// Read back rather than taken from the mint's answer, which carries no body at all —
/// so the value is only knowable by asking again.
#[tokio::test]
async fn a_key_is_minted_where_none_is_ours_yet() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, SOMEONE_ELSES),
        Answer::reply(200, SIGNED_IN),
        Answer::reply(204, ""),
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, OURS_TOO),
    ]);
    let held = reader(&fake).api_key().await;

    // The value is not in the message: it is a credential, and a failing
    // assertion prints its message into the run's log.
    assert!(
        held.is_ok_and(|key| key == "ours"),
        "the key already filed under our name was not handed back"
    );
    let minted = fake
        .requests()
        .into_iter()
        .find(|asked| asked.method == Method::Post && asked.url.contains("/Auth/Keys"));
    assert!(
        minted.is_some_and(|asked| asked.url.contains("App=lemonfiber")),
        "the key was not minted under lemonfiber's own name"
    );
}

/// A mint the server takes but does not list is reported rather than called done.
#[tokio::test]
async fn a_key_that_does_not_appear_after_minting_is_reported() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, SOMEONE_ELSES),
        Answer::reply(200, SIGNED_IN),
        Answer::reply(204, ""),
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, SOMEONE_ELSES),
    ]);
    assert!(matches!(
        reader(&fake).api_key().await,
        Err(Failure::Refused { .. })
    ));
}

/// A key list that cannot be read is a failure, not an absent key.
#[tokio::test]
async fn a_key_list_that_cannot_be_read_is_reported() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "not json"),
    ]);
    assert!(reader(&fake).api_key().await.is_err());
}

/// The account's own policy, as the media server hands it back.
const ALREADY_HELD: &str = r#"{"Policy":{"EnableAllFolders":true,"EnabledFolders":["films"],"BlockUnratedItems":["Movie"],"MaxParentalRating":null}}"#;

/// A choice nobody made leaves the answer the account already had standing.
///
/// Each of the three parts of `Allowed` may be absent, and absent means the household's
/// own answer holds. Writing a value for one nobody named takes that answer away behind
/// their back: setting an age limit would otherwise decide, on its way past, what becomes
/// of everything the server holds no rating for — and the policy is posted whole, so
/// every field not written over is a field written back.
#[tokio::test]
async fn a_choice_nobody_made_leaves_the_accounts_own_answer_standing() {
    let fake = Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, SIGNED_IN),
        ),
        (Method::Post, "/Policy", Answer::reply(204, "")),
        (Method::Get, "/Users/", Answer::reply(200, ALREADY_HELD)),
    ]);

    let only_a_limit = Allowed {
        libraries: None,
        age_limit: Some(12),
        unrated: None,
    };
    assert!(
        reader(&fake).allow("member-4", &only_a_limit).await.is_ok(),
        "the account was not written"
    );

    let written: serde_json::Value = fake
        .requests()
        .into_iter()
        .find(|request| request.url.ends_with("/Policy"))
        .and_then(|request| request.body)
        .and_then(|body| serde_json::from_str(&body).ok())
        .unwrap_or_default();

    assert_eq!(
        written.get("MaxParentalRating"),
        Some(&serde_json::json!(12)),
        "the one thing chosen was not written: {written}"
    );
    assert_eq!(
        written.get("BlockUnratedItems"),
        Some(&serde_json::json!(["Movie"])),
        "an offer saying nothing about unrated content changed it anyway: {written}"
    );
    assert_eq!(
        written.get("EnableAllFolders"),
        Some(&serde_json::json!(true)),
        "an offer naming no libraries changed which ones are open: {written}"
    );
}
