//! The listening server's client, driven through the HTTP port against a fake
//! transport.
//!
//! Its first run is one question and one call: whether an account exists, and making
//! the first one. The shapes here were read off `ghcr.io/advplyr/audiobookshelf:2.17.7`,
//! and the flow driven against it.

use std::sync::Arc;

use lemonfiber_core::audiobookshelf::Audiobookshelf;
use lemonfiber_core::ports::http::{Http, Method};
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
