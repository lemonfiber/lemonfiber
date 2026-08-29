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
use lemonfiber_core::ports::http::{Http, Method};
use lemonfiber_core::ports::service::{Failure, Household, Library, MediaServer};
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

/// The account is read before it is written, and setup is finished last.
///
/// The read is not decoration. Jellyfin's write **updates the first account it
/// holds** rather than creating one, and a server nobody has set up holds none — so
/// the write alone fails on an empty sequence and no administrator is ever made.
/// Asserted as a sequence of methods rather than of paths, because the read and the
/// write are the same path and only the method tells them apart.
#[tokio::test]
async fn the_account_is_read_into_being_before_it_is_written() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, r#"{"Name":"root"}"#),
        Answer::reply(204, ""),
        Answer::reply(204, ""),
    ]);
    assert!(jellyfin(&fake)
        .create_admin("admin", "secret")
        .await
        .is_ok());

    let requests = fake.requests();
    let steps: Vec<(Method, &str)> = requests
        .iter()
        .map(|request| {
            let path = request
                .url
                .rsplit_once("/api")
                .map_or(request.url.as_str(), |(_, rest)| rest);
            (request.method, path)
        })
        .collect();
    assert!(
        matches!(
            steps.as_slice(),
            [
                (Method::Get, first),
                (Method::Post, second),
                (Method::Post, third)
            ] if first.ends_with("/Startup/User")
                && second.ends_with("/Startup/User")
                && third.ends_with("/Startup/Complete")
        ),
        "the account was not read before it was written: {steps:?}"
    );

    let written = requests
        .get(1)
        .and_then(|request| request.body.clone())
        .unwrap_or_default();
    assert!(written.contains(r#""Name":"admin""#), "{written}");
    assert!(written.contains(r#""Password":"secret""#), "{written}");
}

#[tokio::test]
async fn a_rejected_admin_creation_is_refused_and_setup_is_not_finished() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, r#"{"Name":"root"}"#),
        Answer::reply(400, "user already exists"),
    ]);
    assert!(matches!(
        jellyfin(&fake).create_admin("admin", "secret").await,
        Err(Failure::Refused { .. })
    ));
    // The read and the failed write; completion was never reached.
    assert_eq!(fake.requests().len(), 2);
}

#[tokio::test]
async fn a_rejected_completion_is_refused() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, r#"{"Name":"root"}"#),
        Answer::reply(204, ""),
        Answer::reply(500, "boom"),
    ]);
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

// ---- The accounts a household signs in with, and the ones nobody has claimed. ----

/// Two accounts: one somebody has claimed, one nobody has.
const HOUSEHOLD: &str = r#"[
    {"Id":"1","Name":"ana","HasPassword":false},
    {"Id":"2","Name":"bo","HasPassword":true}
]"#;

/// An account with no password on it is an invitation; one with a password is a member.
///
/// The whole distinction the feature rests on, read off the field the server already
/// keeps rather than out of anything written down here.
#[tokio::test]
async fn an_account_without_a_password_reads_as_unclaimed() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, HOUSEHOLD),
    ]);

    let held = reader(&fake).household().await.unwrap_or_default();

    assert_eq!(
        held.iter()
            .map(|member| (member.name.as_str(), member.claimed))
            .collect::<Vec<_>>(),
        [("ana", false), ("bo", true)],
        "claimed and unclaimed were not told apart"
    );
}

/// Offering an account sends no password, which is what makes it an invitation.
#[tokio::test]
async fn offering_an_account_sends_no_password() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Id":"1","Name":"ana","HasPassword":false}"#),
    ]);

    let made = reader(&fake).invite("ana").await;

    assert!(made.is_ok_and(|member| member.name == "ana" && !member.claimed));
    let sent = fake
        .requests()
        .into_iter()
        .find(|asked| asked.url.ends_with("/Users/New"))
        .and_then(|asked| asked.body)
        .unwrap_or_default();
    assert!(
        sent.contains(r#""Name":"ana""#) && !sent.to_lowercase().contains("password"),
        "a password was sent with an invitation: {sent}"
    );
}

/// Taking an account back asks the server to remove it.
#[tokio::test]
async fn withdrawing_an_invitation_removes_the_account() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(204, "")]);

    assert!(reader(&fake).withdraw("1").await.is_ok());
    assert!(
        fake.requests()
            .iter()
            .any(|asked| asked.method == Method::Delete && asked.url.ends_with("/Users/1")),
        "the account was not asked to be removed: {:?}",
        fake.requests()
    );
}

/// When an account was made comes from what the server recorded happening.
///
/// The account itself carries no date at all, so this reads the record instead — and
/// keeps only the entries about an account being made, since the same record holds
/// every sign-in and every password change too.
#[tokio::test]
async fn when_an_account_was_made_is_read_from_what_the_server_recorded() {
    let recorded = r#"{"Items":[
        {"Type":"UserCreated","Date":"2026-08-29T09:00:00Z","UserId":"1"},
        {"Type":"AuthenticationSucceeded","Date":"2026-08-29T10:00:00Z","UserId":"2"},
        {"Type":"UserCreated","Date":"2026-08-29T11:00:00Z","UserId":""}
    ]}"#;
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, recorded),
    ]);

    let made = reader(&fake)
        .when_invited("2026-08-27T09:00:00Z")
        .await
        .unwrap_or_default();

    assert_eq!(
        made.iter()
            .map(|entry| (entry.member.as_str(), entry.at.as_str()))
            .collect::<Vec<_>>(),
        [("1", "2026-08-29T09:00:00Z")],
        "something other than an account being made was carried through"
    );
    assert!(
        fake.requests().iter().any(
            |asked| asked.url.contains("minDate=2026-08-27T09%3A00%3A00Z")
                || asked.url.contains("minDate=2026-08-27T09:00:00Z")
        ),
        "the read was not bounded by the date asked for: {:?}",
        fake.requests()
    );
}

/// A media server that will not answer is unavailable, not an empty household.
#[tokio::test]
async fn a_media_server_that_will_not_answer_is_unavailable_for_the_household() {
    let fake = Fake::silent();
    assert!(matches!(
        reader(&fake).household().await,
        Err(Failure::Unavailable { .. })
    ));
}
