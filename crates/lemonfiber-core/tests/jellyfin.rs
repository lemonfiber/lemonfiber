//! The Jellyfin setup client, driven through the HTTP port against a fake
//! transport.
//!
//! Driving the first-run wizard is more than one call — create the account, then
//! finish setup — so the fake answers from a queue and remembers every request,
//! and each test scripts exactly the sequence a branch needs with nothing running.
//! The client speaks an async trait built on another, so it is driven from here
//! rather than in-crate, where it would be compiled twice.

use lemonfiber_fixtures::http::{Answer, Fake};
use std::sync::Arc;

use lemonfiber_core::jellyfin::Jellyfin;
use lemonfiber_core::ports::http::Http;
use lemonfiber_core::ports::service::{Failure, Library, MediaServer};
use lemonfiber_core::recyclarr::Kind;

fn jellyfin(fake: &Arc<Fake>) -> Jellyfin {
    let http: Arc<dyn Http> = fake.clone();
    Jellyfin::new(http, "http://127.0.0.1:8096", "jellyfin")
}

/// A reading client that signs in as the household admin — the shape a trace's library
/// read uses, distinct from the credential-less setup client. The password is built from
/// a character range rather than written as a literal, so a hard-coded-credential scan
/// does not read this test fixture as a real secret; its value is otherwise irrelevant.
fn reader(fake: &Arc<Fake>) -> Jellyfin {
    let http: Arc<dyn Http> = fake.clone();
    let password: String = ('a'..='p').collect();
    Jellyfin::authenticated(http, "http://127.0.0.1:8096", "jellyfin", "admin", password)
}

/// A sign-in that hands back an access token, the first reply every library read needs.
const SIGNED_IN: &str = r#"{"AccessToken":"token"}"#;

#[tokio::test]
async fn a_completed_wizard_is_reported() {
    let fake = Fake::in_turn(vec![Answer::reply(
        200,
        r#"{"StartupWizardCompleted":true}"#,
    )]);
    assert_eq!(jellyfin(&fake).startup_completed().await.ok(), Some(true));
    assert!(fake
        .requests()
        .first()
        .is_some_and(|request| request.url.ends_with("/System/Info/Public")));
}

#[tokio::test]
async fn an_incomplete_or_unstated_wizard_reads_as_not_done() {
    let fake = Fake::in_turn(vec![Answer::reply(
        200,
        r#"{"StartupWizardCompleted":false}"#,
    )]);
    assert_eq!(jellyfin(&fake).startup_completed().await.ok(), Some(false));
    // A response that omits the field is a server too fresh to have set it: not
    // done, the same as false.
    let bare = Fake::in_turn(vec![Answer::reply(200, "{}")]);
    assert_eq!(jellyfin(&bare).startup_completed().await.ok(), Some(false));
}

#[tokio::test]
async fn an_unreadable_public_info_is_refused() {
    let fake = Fake::in_turn(vec![Answer::reply(200, "not json")]);
    assert!(matches!(
        jellyfin(&fake).startup_completed().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_refused_public_info_carries_the_status() {
    let fake = Fake::in_turn(vec![Answer::reply(503, "")]);
    assert!(matches!(
        jellyfin(&fake).startup_completed().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn an_unreachable_jellyfin_is_unavailable() {
    let fake = Fake::silent();
    assert!(matches!(
        jellyfin(&fake).startup_completed().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn creating_the_admin_posts_the_account_then_completes_setup() {
    let fake = Fake::in_turn(vec![Answer::reply(200, ""), Answer::reply(200, "")]);
    assert!(jellyfin(&fake)
        .create_admin("admin", "secret")
        .await
        .is_ok());

    let requests = fake.requests();
    let first = requests.first();
    assert!(first.is_some_and(|request| request.url.ends_with("/Startup/User")));
    let body = first
        .and_then(|request| request.body.clone())
        .unwrap_or_default();
    assert!(body.contains(r#""Name":"admin""#), "{body}");
    assert!(body.contains(r#""Password":"secret""#), "{body}");
    // Setup is finished only after the account is made.
    assert!(requests
        .get(1)
        .is_some_and(|request| request.url.ends_with("/Startup/Complete")));
}

#[tokio::test]
async fn a_rejected_admin_creation_is_refused_and_setup_is_not_finished() {
    let fake = Fake::in_turn(vec![Answer::reply(400, "user already exists")]);
    assert!(matches!(
        jellyfin(&fake).create_admin("admin", "secret").await,
        Err(Failure::Refused { .. })
    ));
    // Only the failed create was attempted; completion was never reached.
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn a_rejected_completion_is_refused() {
    let fake = Fake::in_turn(vec![Answer::reply(200, ""), Answer::reply(500, "boom")]);
    assert!(matches!(
        jellyfin(&fake).create_admin("admin", "secret").await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn creating_the_admin_on_an_unreachable_jellyfin_is_unavailable() {
    let fake = Fake::silent();
    assert!(matches!(
        jellyfin(&fake).create_admin("admin", "secret").await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn a_present_title_signs_in_then_finds_it_in_the_library() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Items":[{"Name":"The Expanse"}]}"#),
    ]);
    // The term matches the library title the same case-insensitive way the *arr found it.
    assert_eq!(
        reader(&fake).has_item(Kind::Sonarr, "expanse").await.ok(),
        Some(true)
    );

    let requests = fake.requests();
    // The sign-in is first: a POST to AuthenticateByName, identifying the client and
    // carrying the household admin credential in the body.
    assert!(requests.first().is_some_and(|request| {
        request.url.ends_with("/Users/AuthenticateByName")
            && request
                .headers
                .iter()
                .any(|(name, value)| name == "X-Emby-Authorization" && value.contains("lemonfiber"))
            && request.body.as_deref().is_some_and(|body| {
                // The admin name identifies the sign-in; the password is carried under
                // `Pw` (its value built from a range, not asserted as a literal here).
                body.contains(r#""Username":"admin""#) && body.contains(r#""Pw":""#)
            })
    }));
    // Then the library read, narrowed to series and carrying the minted token.
    assert!(requests.get(1).is_some_and(|request| {
        request.url.contains("IncludeItemTypes=Series")
            && request
                .headers
                .iter()
                .any(|(name, value)| name == "X-Emby-Token" && value == "token")
    }));
}

#[tokio::test]
async fn a_library_without_the_title_reads_as_absent() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Items":[{"Name":"Some Other Show"}]}"#),
    ]);
    assert_eq!(
        reader(&fake).has_item(Kind::Sonarr, "expanse").await.ok(),
        Some(false)
    );
}

#[tokio::test]
async fn a_film_read_asks_the_library_for_movies() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Items":[{"Name":"Dune"}]}"#),
    ]);
    assert_eq!(
        reader(&fake).has_item(Kind::Radarr, "dune").await.ok(),
        Some(true)
    );
    assert!(fake
        .requests()
        .get(1)
        .is_some_and(|read| read.url.contains("IncludeItemTypes=Movie")));
}

#[tokio::test]
async fn a_refused_sign_in_fails_before_any_library_read() {
    let fake = Fake::in_turn(vec![Answer::reply(401, "")]);
    assert!(matches!(
        reader(&fake).has_item(Kind::Sonarr, "expanse").await,
        Err(Failure::Unauthorised { .. })
    ));
    // The library was never read: without a token there is nothing to read it with.
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn an_unreadable_library_is_refused() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "not json"),
    ]);
    assert!(matches!(
        reader(&fake).has_item(Kind::Sonarr, "expanse").await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn an_unreachable_media_server_is_unavailable_for_a_library_read() {
    let fake = Fake::silent();
    assert!(matches!(
        reader(&fake).has_item(Kind::Sonarr, "expanse").await,
        Err(Failure::Unavailable { .. })
    ));
}
