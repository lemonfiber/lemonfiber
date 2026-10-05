//! Revoking the key filed under lemonfiber's name on the media server, and choosing an
//! identity.

use super::{reader, OURS_TOO, SIGNED_IN, SOMEONE_ELSES};
use lemonfiber_core::ports::http::Method;
use lemonfiber_core::ports::service::Allowed;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::Household;

/// The key filed under lemonfiber's name is revoked, and nobody else's.
///
/// An API key on this server administers all of it, and nothing in the stack reads
/// lemonfiber's. Seerr's key beside it is Seerr's, and is left alone.
#[tokio::test]
async fn the_key_filed_under_our_name_is_revoked_and_no_other() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, OURS_TOO),
        Answer::reply(204, ""),
    ]);
    let revoked = reader(&fake).revoke_our_key().await;

    assert!(matches!(revoked, Ok(true)), "{revoked:?}");
    let deleted: Vec<String> = fake
        .requests()
        .into_iter()
        .filter(|asked| asked.method == Method::Delete)
        .map(|asked| asked.url)
        .collect();
    assert_eq!(deleted.len(), 1, "{deleted:?}");
    assert!(
        deleted.iter().all(|url| url.ends_with("/Auth/Keys/ours")),
        "a key that was not ours was revoked: {deleted:?}"
    );
}

/// With no key of ours there is nothing to revoke, and nothing is asked to be.
#[tokio::test]
async fn with_no_key_of_ours_nothing_is_revoked() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, SOMEONE_ELSES),
    ]);

    assert!(matches!(reader(&fake).revoke_our_key().await, Ok(false)));
    assert!(
        !fake
            .requests()
            .iter()
            .any(|asked| asked.method == Method::Delete),
        "somebody else's key was revoked"
    );
}

/// A revocation the server refuses is reported rather than called done.
#[tokio::test]
async fn a_revocation_the_server_refuses_is_reported() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, OURS_TOO),
        Answer::reply(500, ""),
    ]);
    assert!(reader(&fake).revoke_our_key().await.is_err());
}

/// A key list that cannot be read is a failure, not an absent key.
#[tokio::test]
async fn a_key_list_that_cannot_be_read_is_reported() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "not json"),
    ]);
    assert!(reader(&fake).revoke_our_key().await.is_err());
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

/// Two decline keys, one with nothing in it, and Seerr's: what the server lists for the
/// decline service's dates.
const DECLINE_KEYS: &str = r#"{"Items":[
  {"AppName":"lemonfiber-decline","AccessToken":"first","DateCreated":"2026-10-05T00:00:00Z","DateLastActivity":"2026-10-05T01:00:00Z"},
  {"AppName":"lemonfiber-decline","AccessToken":"second","DateCreated":"2026-10-05T02:00:00Z"},
  {"AppName":"lemonfiber-decline","AccessToken":"","DateCreated":"2026-10-05T03:00:00Z"},
  {"AppName":"Jellyseerr","AccessToken":"seerr","DateCreated":"2026-10-05T04:00:00Z","DateLastActivity":"2026-10-05T05:00:00Z"}
]}"#;

/// Every key filed under the decline service's name is dated, not only the newest, so a
/// second key filed beside it cannot hide the first one's use; a key with nothing in it
/// and another service's are not.
#[tokio::test]
async fn every_key_filed_under_the_decline_service_s_name_is_dated() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, DECLINE_KEYS),
    ]);

    let dated = reader(&fake)
        .dated(lemonfiber_core::jellyfin::DECLINE_APP)
        .await
        .ok();

    assert_eq!(
        dated,
        Some(vec![
            lemonfiber_core::jellyfin::Dated {
                created: Some("2026-10-05T00:00:00Z".to_owned()),
                last_used: Some("2026-10-05T01:00:00Z".to_owned()),
            },
            lemonfiber_core::jellyfin::Dated {
                created: Some("2026-10-05T02:00:00Z".to_owned()),
                last_used: None,
            },
        ])
    );
}

/// A key list that cannot be read is a failure, not a list with no decline key in it.
#[tokio::test]
async fn an_unreadable_key_list_is_no_list_of_dates() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "not json"),
    ]);

    assert!(reader(&fake)
        .dated(lemonfiber_core::jellyfin::DECLINE_APP)
        .await
        .is_err());
}
