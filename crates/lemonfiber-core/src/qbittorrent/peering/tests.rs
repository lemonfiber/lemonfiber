use std::sync::Arc;

use lemonfiber_fixtures::http::Fake;

use super::Qbittorrent;
use crate::ports::service::{Failure, Peering};
use crate::test_support::a_password;

/// A client answering each call from `replies` in order, and the transport it asks.
fn writing(replies: Vec<(u16, &'static str)>) -> (Qbittorrent, Arc<Fake>) {
    let fake = Fake::scripted(replies);
    let client = Qbittorrent::authenticated(fake.clone(), "http://127.0.0.1:8080", a_password());
    (client, fake)
}

const LOGGED_IN: (u16, &str) = (200, "Ok.");

#[tokio::test]
async fn the_port_it_takes_peers_on_is_the_one_it_listens_on() {
    let (client, _) = writing(vec![(200, r#"{"listen_port":51413}"#)]);
    assert_eq!(client.peer_port().await.ok(), Some(Some(51413)));
}

#[tokio::test]
async fn a_client_set_to_no_forwarded_port_takes_peers_on_none() {
    let (client, _) = writing(vec![(200, r#"{"listen_port":0}"#)]);
    assert_eq!(client.peer_port().await.ok(), Some(None));
}

#[tokio::test]
async fn a_forwarded_port_is_set_and_read_back() {
    let (client, fake) = writing(vec![
        LOGGED_IN,
        (200, ""),
        (200, r#"{"listen_port":51413}"#),
    ]);
    assert!(client.set_peer_port(Some(51413)).await.is_ok());
    assert!(fake.requests().iter().any(|request| request
        .body
        .as_deref()
        .is_some_and(|body| body.contains("51413"))));
}

#[tokio::test]
async fn no_forwarded_port_sets_the_client_to_none_and_reads_it_back() {
    let (client, fake) = writing(vec![LOGGED_IN, (200, ""), (200, r#"{"listen_port":0}"#)]);
    assert!(client.set_peer_port(None).await.is_ok());
    assert!(fake.requests().iter().any(|request| request
        .body
        .as_deref()
        .is_some_and(|body| body.contains("%22listen_port%22%3A0%7D")
            || body.contains(r#""listen_port":0}"#))));
}

#[tokio::test]
async fn a_client_still_on_a_port_after_being_set_to_none_is_a_failure() {
    let (client, _) = writing(vec![LOGGED_IN, (200, ""), (200, r#"{"listen_port":6881}"#)]);
    assert!(matches!(
        client.set_peer_port(None).await,
        Err(Failure::Refused { .. })
    ));
}
