//! The media server's own setup and administrator.

use super::jellyfin;
use lemonfiber_core::ports::http::Method;
use lemonfiber_core::ports::service::Failure;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::MediaServer;
use lemonfiber_testing::a_word;

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
    let password = a_word();
    let fake = Fake::in_turn(vec![
        Answer::reply(200, r#"{"Name":"root"}"#),
        Answer::reply(204, ""),
        Answer::reply(204, ""),
    ]);
    assert!(jellyfin(&fake)
        .create_admin("admin", &password)
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
    assert!(
        written.contains(r#""Name":"admin""#),
        "the account written is not the one asked for"
    );
    assert!(
        written.contains(&format!(r#""Password":"{password}""#)),
        "the password written is not the one asked for"
    );
}

#[tokio::test]
async fn a_rejected_admin_creation_is_refused_and_setup_is_not_finished() {
    let password = a_word();
    let fake = Fake::in_turn(vec![
        Answer::reply(200, r#"{"Name":"root"}"#),
        Answer::reply(400, "user already exists"),
    ]);
    assert!(matches!(
        jellyfin(&fake).create_admin("admin", &password).await,
        Err(Failure::Refused { .. })
    ));
    // The read and the failed write; completion was never reached.
    assert_eq!(fake.requests().len(), 2);
}

#[tokio::test]
async fn a_rejected_completion_is_refused() {
    let password = a_word();
    let fake = Fake::in_turn(vec![
        Answer::reply(200, r#"{"Name":"root"}"#),
        Answer::reply(204, ""),
        Answer::reply(500, "boom"),
    ]);
    assert!(matches!(
        jellyfin(&fake).create_admin("admin", &password).await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn creating_the_admin_on_an_unreachable_jellyfin_is_unavailable() {
    let password = a_word();
    let fake = Fake::silent();
    assert!(matches!(
        jellyfin(&fake).create_admin("admin", &password).await,
        Err(Failure::Unavailable { .. })
    ));
}
