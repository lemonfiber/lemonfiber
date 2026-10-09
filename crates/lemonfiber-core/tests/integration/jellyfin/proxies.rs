//! The address Jellyfin trusts to name the client, written whole and held to it.

use super::{reader, SIGNED_IN};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;

const BEFORE: &str = r#"{"KnownProxies":[],"EnableRemoteAccess":true}"#;
const AFTER: &str = r#"{"KnownProxies":["10.80.96.18"],"EnableRemoteAccess":true}"#;

/// The door alone is trusted, the rest of the configuration is written back as it was,
/// and what was kept is read again.
#[tokio::test]
async fn the_door_alone_is_trusted_and_the_rest_is_written_back_whole() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, BEFORE),
        Answer::reply(204, ""),
        Answer::reply(200, AFTER),
    ]);
    assert!(reader(&fake).trust_only("10.80.96.18").await.is_ok());
    let written = fake
        .requests()
        .into_iter()
        .find(|request| {
            request.method == Method::Post && request.url.ends_with("/System/Configuration/network")
        })
        .and_then(|request| request.body)
        .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok());
    assert_eq!(
        written,
        serde_json::from_str::<serde_json::Value>(AFTER).ok()
    );
}

/// A list the server did not keep is a failure, and so is nothing to trust.
#[tokio::test]
async fn a_list_not_kept_and_an_empty_address_are_refused() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, BEFORE),
        Answer::reply(204, ""),
        Answer::reply(200, BEFORE),
    ]);
    let client = reader(&fake);
    assert!(client.trust_only("10.80.96.18").await.is_err());
    assert!(client.trust_only(" ").await.is_err());
}

/// Each step of trusting the door fails where the server is gone, refuses, or keeps a
/// configuration that is not an object, and so does asking it to restart.
#[tokio::test]
async fn every_step_of_trusting_the_door_fails_where_the_server_is_gone_or_refuses() {
    let signed = || Answer::reply(200, SIGNED_IN);
    let before = || Answer::reply(200, BEFORE);
    let steps = vec![
        vec![signed(), Answer::Silent],
        vec![signed(), Answer::reply(200, "[]")],
        vec![signed(), before(), Answer::Silent],
        vec![signed(), before(), Answer::reply(500, "")],
        vec![signed(), before(), Answer::reply(204, ""), Answer::Silent],
    ];
    for (at, answers) in steps.into_iter().enumerate() {
        let fake = Fake::in_turn(answers);
        let trusted = reader(&fake).trust_only("10.80.96.18").await;
        assert!(trusted.is_err(), "step {at} was trusted");
    }
    let fake = Fake::in_turn(vec![signed(), Answer::Silent]);
    assert!(reader(&fake).restart().await.is_err());
}

/// What is trusted now is read, and a restart is asked for by name.
#[tokio::test]
async fn what_is_trusted_is_read_and_a_restart_is_asked_for() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, AFTER),
        Answer::reply(204, ""),
    ]);
    let client = reader(&fake);
    assert_eq!(
        client.known_proxies().await.ok(),
        Some(vec!["10.80.96.18".to_owned()])
    );
    assert!(client.restart().await.is_ok());
    assert!(fake
        .requests()
        .iter()
        .any(|request| request.url.ends_with("/System/Restart")));
}
