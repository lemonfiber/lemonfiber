//! The qBittorrent client, driven through the HTTP port against a fake transport.
//!
//! Replacing the password is three calls in sequence — log in, set, log in again
//! to confirm — so the fake answers from a scripted queue, one reply per call,
//! and records what it was asked. The client is an `#[async_trait]` built on
//! another, so it is exercised from here rather than from a `#[cfg(test)]` module,
//! where async-trait code is compiled twice and its coverage counted wrong.

use lemonfiber_fixtures::http::{Answer, Fake};
use std::sync::Arc;

use lemonfiber_core::ports::http::Http;
use lemonfiber_core::ports::service::Failure;
use lemonfiber_core::qbittorrent::{temporary_password, Qbittorrent};
use lemonfiber_core::seed::{wire_qbittorrent_password, State};

fn qbittorrent(fake: &Arc<Fake>) -> Qbittorrent {
    let http: Arc<dyn Http> = fake.clone();
    Qbittorrent::new(http, "http://127.0.0.1:8081")
}

/// The login answer qBittorrent gives when the password was right.
/// What qBittorrent answers a write it accepted.
fn ok() -> Answer {
    Answer::reply(200, "Ok.")
}

#[test]
fn the_temporary_password_is_read_from_the_log() {
    let log = "\
The WebUI administrator username is: admin
The WebUI administrator password was not set. A temporary password is provided for this session: tempword
You should set your own password in program preferences.
";
    assert_eq!(temporary_password(log).as_deref(), Some("tempword"));
}

#[test]
fn the_most_recent_announcement_wins() {
    // A restart announces a fresh password; the later one is the live one.
    let log = "\
A temporary password is provided for this session: staleword
A temporary password is provided for this session: freshword
";
    assert_eq!(temporary_password(log).as_deref(), Some("freshword"));
}

#[test]
fn a_truncated_announcement_is_no_password() {
    assert_eq!(
        temporary_password("A temporary password is provided for this session:"),
        None
    );
    assert_eq!(
        temporary_password("A temporary password is provided for this session:   "),
        None
    );
}

#[test]
fn a_log_without_the_announcement_has_no_password() {
    assert_eq!(
        temporary_password("nothing to see here\nstarting up\n"),
        None
    );
}

#[tokio::test]
async fn replacing_the_password_authenticates_sets_and_confirms() {
    let fake = Fake::in_turn(vec![ok(), Answer::reply(200, ""), ok()]);
    let outcome = qbittorrent(&fake)
        .replace_password("tempword", "freshword")
        .await;
    assert!(outcome.is_ok(), "{outcome:?}");

    let requests = fake.requests();
    assert_eq!(
        requests.len(),
        3,
        "log in, set, then log in again to confirm"
    );
    // Authenticate with the current password.
    assert!(requests
        .first()
        .is_some_and(|request| request.url.ends_with("/api/v2/auth/login")
            && request
                .body
                .as_deref()
                .is_some_and(|body| body.contains("tempword"))));
    // Set the new password.
    assert!(requests.get(1).is_some_and(|request| request
        .url
        .ends_with("/api/v2/app/setPreferences")
        && request
            .body
            .as_deref()
            .is_some_and(|body| body.contains("web_ui_password") && body.contains("freshword"))));
    // Confirm by authenticating with the new password.
    assert!(requests
        .get(2)
        .is_some_and(|request| request.url.ends_with("/api/v2/auth/login")
            && request
                .body
                .as_deref()
                .is_some_and(|body| body.contains("freshword"))));
}

#[tokio::test]
async fn a_wrong_current_password_is_unauthorised() {
    let fake = Fake::in_turn(vec![Answer::reply(200, "Fails.")]);
    assert!(matches!(
        qbittorrent(&fake)
            .replace_password("wrongword", "freshword")
            .await,
        Err(Failure::Unauthorised { .. })
    ));
}

#[tokio::test]
async fn a_login_ban_is_unauthorised() {
    let fake = Fake::in_turn(vec![Answer::reply(403, "")]);
    assert!(matches!(
        qbittorrent(&fake)
            .replace_password("tempword", "freshword")
            .await,
        Err(Failure::Unauthorised { .. })
    ));
}

#[tokio::test]
async fn a_login_that_answers_unexpectedly_is_refused() {
    // Not a wrong password and not a ban — an answer lemonfiber does not
    // recognise, carried through with its status rather than guessed at.
    let fake = Fake::in_turn(vec![Answer::reply(500, "")]);
    let detail = match qbittorrent(&fake)
        .replace_password("tempword", "freshword")
        .await
    {
        Err(Failure::Refused { detail, .. }) => Some(detail),
        _ => None,
    };
    assert_eq!(detail.as_deref(), Some("HTTP 500"));
}

#[tokio::test]
async fn a_change_refused_as_unauthorised_is_unauthorised() {
    // The set itself can be refused for want of authorisation — a 403 there is a
    // rejected credential, not an unrecognised answer.
    let fake = Fake::in_turn(vec![ok(), Answer::reply(403, "")]);
    assert!(matches!(
        qbittorrent(&fake)
            .replace_password("tempword", "freshword")
            .await,
        Err(Failure::Unauthorised { .. })
    ));
}

#[tokio::test]
async fn a_refused_change_carries_the_services_own_words() {
    let fake = Fake::in_turn(vec![ok(), Answer::reply(500, "internal error")]);
    let detail = match qbittorrent(&fake)
        .replace_password("tempword", "freshword")
        .await
    {
        Err(Failure::Refused { detail, .. }) => Some(detail),
        _ => None,
    };
    assert!(
        detail.is_some_and(|words| words.contains("500") && words.contains("internal error")),
        "the service's own words survive"
    );
}

#[tokio::test]
async fn a_change_that_does_not_take_is_caught_by_the_confirming_login() {
    // The set was accepted but the new password does not authenticate, so it did
    // not land — caught by the confirming login rather than called done.
    let fake = Fake::in_turn(vec![
        ok(),
        Answer::reply(200, ""),
        Answer::reply(200, "Fails."),
    ]);
    assert!(matches!(
        qbittorrent(&fake)
            .replace_password("tempword", "freshword")
            .await,
        Err(Failure::Unauthorised { .. })
    ));
}

#[tokio::test]
async fn a_qbittorrent_that_is_not_answering_is_unavailable() {
    let fake = Fake::in_turn(vec![Answer::Silent]);
    assert!(matches!(
        qbittorrent(&fake)
            .replace_password("tempword", "freshword")
            .await,
        Err(Failure::Unavailable { .. })
    ));
}

// ---- The seed driver that mints, sets and hands back the password. ----

#[tokio::test]
async fn a_generated_password_is_set_confirmed_and_handed_back() {
    // Login, set, and the confirming login all succeed.
    let fake = Fake::in_turn(vec![ok(), Answer::reply(200, ""), ok()]);
    let random = lemonfiber_fixtures::ports::Chance::exactly(Some(vec![0x11; 24]));

    let (wiring, recorded) =
        wire_qbittorrent_password(&qbittorrent(&fake), &random, "tempword").await;

    assert!(matches!(wiring.state, State::Wired));
    // The value handed back for recording is the one that was set: it appears in
    // the setPreferences request body.
    let password = recorded.unwrap_or_default();
    assert!(!password.is_empty(), "a wired password is handed back");
    let set_body = fake
        .requests()
        .into_iter()
        .find(|request| request.url.ends_with("/app/setPreferences"))
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(
        set_body.contains(&password),
        "the recorded password is the one set"
    );
}

#[tokio::test]
async fn without_randomness_the_password_is_not_set() {
    // Nothing to set, so the client is never even called — and nothing is handed
    // back to record.
    let fake = Fake::in_turn(Vec::new());
    let random = lemonfiber_fixtures::ports::Chance::exactly(None);

    let (wiring, recorded) =
        wire_qbittorrent_password(&qbittorrent(&fake), &random, "tempword").await;

    assert!(matches!(wiring.state, State::Failed { .. }));
    assert_eq!(recorded, None);
    assert!(
        fake.requests().is_empty(),
        "no randomness means no call is made"
    );
}

#[tokio::test]
async fn a_rejected_current_password_fails_and_records_nothing() {
    let fake = Fake::in_turn(vec![Answer::reply(200, "Fails.")]);
    let random = lemonfiber_fixtures::ports::Chance::exactly(Some(vec![0x11; 24]));

    let (wiring, recorded) =
        wire_qbittorrent_password(&qbittorrent(&fake), &random, "wrongword").await;

    assert!(matches!(wiring.state, State::Failed { .. }));
    assert_eq!(recorded, None);
}
