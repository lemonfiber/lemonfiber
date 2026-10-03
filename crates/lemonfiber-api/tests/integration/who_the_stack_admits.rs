//! The door as the web surface assembles it, over the household the stack holds.
//!
//! `door` proves the door against a household a test hands it. These prove the other
//! half: that the household the door asks is the stack's own media server, reached at
//! the address the stack gives it, and opened from what the stack holds when somebody
//! knocks.

use std::fs;

use crate::door;
use door::*;
use lemonfiber_fixtures::http::Answer as Reply;

/// What the media server answers a name and password it recognises with.
const SIGNED_IN: &str = r#"{"AccessToken":"a-session","User":{"Id":"a7f3"}}"#;

/// What it answers when asked whether that member's account still stands.
const STANDING: &str = r#"{"Id":"a7f3","HasPassword":true}"#;

/// A media server that recognises the member, and says their account stands.
fn recognising() -> Arc<Fake> {
    Fake::by_path(vec![
        ("/Users/AuthenticateByName", Reply::reply(200, SIGNED_IN)),
        ("/Users/Me", Reply::reply(200, STANDING)),
    ])
}

/// A media server that recognises nobody.
fn refusing() -> Arc<Fake> {
    Fake::by_path(vec![("/Users/AuthenticateByName", Reply::reply(401, ""))])
}

/// A member's name and password, sent to the door.
async fn knocked(router: axum::Router, password: &str) -> door::Answer {
    asked(
        router,
        "POST",
        SESSION,
        &from_here(),
        &offering_as("ana", password),
    )
    .await
}

#[tokio::test]
async fn a_member_the_media_server_recognises_is_admitted_as_that_member() {
    let transport = recognising();
    let ctx = a_stack("stack-admits", transport.clone());
    seeded(&ctx);
    let (router, admitting) = door_over(&ctx);

    let answer = knocked(router, &hers()).await;

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(
        transport
            .requests()
            .iter()
            .any(|request| request.url.starts_with("http://127.0.0.1:")
                && request.url.ends_with("/Users/AuthenticateByName")
                && request
                    .body
                    .as_deref()
                    .is_some_and(|sent| sent.contains("\"ana\""))),
        "the name was not offered to the stack's media server"
    );
    let Some(token) = Token::mint(&Chance::exactly(Some(vec![b'q'; 32]))) else {
        unreachable!("bytes of the minting width mint a token")
    };
    assert_eq!(
        admitting
            .carried(&carrying(Some(&session(&answer.body))), &token, moment())
            .await,
        Knocking::Known(Caller::Member("a7f3".to_owned()))
    );
    let _ = fs::remove_dir_all(a_directory("stack-admits"));
}

#[tokio::test]
async fn a_pair_the_media_server_does_not_recognise_is_refused() {
    let ctx = a_stack("stack-refuses", refusing());
    seeded(&ctx);
    let (router, _) = door_over(&ctx);

    let answer = knocked(router, &nobodys()).await;

    assert_eq!(answer.status, StatusCode::UNAUTHORIZED, "{}", answer.body);
    assert!(session(&answer.body).is_empty());
    let _ = fs::remove_dir_all(a_directory("stack-refuses"));
}

/// The household is opened from the stack when somebody knocks, so a stack seeded
/// while the surface was already serving admits its members from then on.
#[tokio::test]
async fn a_stack_seeded_while_serving_admits_its_members_from_then_on() {
    let ctx = a_stack("stack-seeded-later", recognising());
    let (router, _) = door_over(&ctx);

    let before = knocked(router.clone(), &hers()).await;
    seeded(&ctx);
    let after = knocked(router, &hers()).await;

    assert_eq!(before.status, StatusCode::UNAUTHORIZED, "{}", before.body);
    assert_eq!(after.status, StatusCode::OK, "{}", after.body);
    let _ = fs::remove_dir_all(a_directory("stack-seeded-later"));
}
