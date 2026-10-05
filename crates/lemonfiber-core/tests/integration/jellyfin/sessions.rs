//! A sign-in made once and carried, rather than made before every request.

use super::{reader, HOUSEHOLD, SIGNED_IN};
use lemonfiber_core::jellyfin::Sessions;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::Household;
use std::sync::Arc;

/// How many times the media server was asked to sign somebody in.
fn sign_ins(fake: &Fake) -> usize {
    fake.requests()
        .iter()
        .filter(|request| request.url.ends_with("/Users/AuthenticateByName"))
        .count()
}

/// Two reads by one client, and a read by a second client remembering the same
/// sessions, sign in once between them.
#[tokio::test]
async fn reads_carry_the_sign_in_they_already_made() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, HOUSEHOLD),
        Answer::reply(200, HOUSEHOLD),
        Answer::reply(200, HOUSEHOLD),
    ]);
    let sessions = Arc::new(Sessions::default());

    let first = reader(&fake).remembering(Arc::clone(&sessions));
    assert!(first.household().await.is_ok());
    assert!(first.household().await.is_ok());
    let second = reader(&fake).remembering(sessions);
    assert!(second.household().await.is_ok());

    assert_eq!(sign_ins(&fake), 1);
}

/// A token the server no longer takes is minted afresh once and the request sent again.
#[tokio::test]
async fn a_token_the_server_no_longer_takes_is_minted_afresh_once() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, HOUSEHOLD),
        Answer::reply(401, ""),
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, HOUSEHOLD),
    ]);
    let client = reader(&fake).remembering(Arc::new(Sessions::default()));

    assert!(client.household().await.is_ok());
    let again = client.household().await;

    assert_eq!(again.map(|held| held.len()).ok(), Some(2));
    assert_eq!(sign_ins(&fake), 2);
}
