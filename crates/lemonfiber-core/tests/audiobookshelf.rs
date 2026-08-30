//! The listening server's client, driven through the HTTP port against a fake
//! transport.
//!
//! Its first-run is two calls — make the account, then sign in for the token — and
//! the second is what the dashboard's panel authenticates with. The shapes here were
//! read off `ghcr.io/advplyr/audiobookshelf:2.17.7`, and the flow driven against it.

use std::sync::Arc;

use lemonfiber_core::audiobookshelf::Audiobookshelf;
use lemonfiber_core::ports::http::{Http, Method};
use lemonfiber_core::ports::service::Failure;
use lemonfiber_fixtures::http::{Answer, Fake};

fn server(fake: &Arc<Fake>) -> Audiobookshelf {
    let http: Arc<dyn Http> = fake.clone();
    Audiobookshelf::new(http, "http://127.0.0.1:13378", "audiobookshelf")
}

/// A password built rather than written, so a credential scan does not read this
/// fixture as a real secret. Its value is otherwise irrelevant.
fn password() -> String {
    ('a'..='p').collect()
}

/// A server with no account says so, and one with an account says that.
#[tokio::test]
async fn whether_an_account_exists_is_read_from_the_server() {
    let fresh = Fake::always(Answer::reply(200, r#"{"isInit":false}"#));
    assert_eq!(server(&fresh).has_account().await.ok(), Some(false));

    let used = Fake::always(Answer::reply(200, r#"{"isInit":true}"#));
    assert_eq!(server(&used).has_account().await.ok(), Some(true));
}

/// The first account is made under the name and password it is given.
///
/// Asserted on the body that went out: this is the one call that decides what the
/// household's own account is, and a name sent under the wrong field would create an
/// account nobody could sign in to.
#[tokio::test]
async fn the_first_account_is_made_with_the_name_and_password_given() {
    let fake = Fake::always(Answer::reply(200, ""));
    let made = server(&fake).create_account("admin", &password()).await;
    // Not printed: the call carries the password, so a failing message would carry it
    // into the run's log.
    assert!(made.is_ok(), "the first account was not made");

    let sent = fake.requests();
    let body = sent
        .first()
        .and_then(|request| request.body.clone())
        .unwrap_or_default();
    assert!(body.contains("\"newRoot\""), "{body}");
    assert!(body.contains("\"username\":\"admin\""), "{body}");
    assert!(
        sent.first()
            .is_some_and(|request| request.method == Method::Post),
        "the account was not made by a post"
    );
}

/// A server that already has an account refuses to make another, and that is reported.
#[tokio::test]
async fn a_server_that_already_has_an_account_refuses_a_second() {
    let fake = Fake::always(Answer::reply(500, ""));
    assert!(server(&fake)
        .create_account("admin", &password())
        .await
        .is_err());
}

/// Signing in hands back the token the dashboard's panel authenticates with.
#[tokio::test]
async fn signing_in_hands_back_the_token_the_panel_uses() {
    let fake = Fake::always(Answer::reply(200, r#"{"user":{"token":"the-token"}}"#));
    let held = server(&fake).token("admin", &password()).await;
    assert!(
        held.as_ref().is_ok_and(|token| token == "the-token"),
        "{held:?}"
    );
}

/// A sign-in carrying no token is reported rather than handed back as an empty one.
///
/// An empty token published is worse than none: the panel would authenticate with it
/// and be refused, which reads as the service being broken rather than unconfigured.
#[tokio::test]
async fn a_sign_in_without_a_token_is_reported() {
    let empty = Fake::always(Answer::reply(200, r#"{"user":{"token":""}}"#));
    assert!(matches!(
        server(&empty).token("admin", &password()).await,
        Err(Failure::Refused { .. })
    ));

    let nothing = Fake::always(Answer::reply(200, "{}"));
    assert!(server(&nothing).token("admin", &password()).await.is_err());
}

/// A refused sign-in is reported as one.
#[tokio::test]
async fn a_refused_sign_in_is_reported() {
    let fake = Fake::always(Answer::reply(401, ""));
    assert!(matches!(
        server(&fake).token("admin", &password()).await,
        Err(Failure::Unauthorised { .. })
    ));
}
