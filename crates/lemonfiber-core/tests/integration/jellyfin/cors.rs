//! Jellyfin's cross-origin allow-list: named as one origin, written back whole, and
//! held to what the server kept.

use super::{reader, SIGNED_IN};
use lemonfiber_core::ports::http::Method;
use lemonfiber_fixtures::http::{Answer, Fake};

/// The front door's origin, as the household is handed it.
const DOOR: &str = "http://nas.local:5055";

/// A server configuration as Jellyfin answers it, with its allow-list at the default.
const OPEN: &str = r#"{"ServerName":"nas","CorsHosts":["*"],"EnableMetrics":false}"#;

/// The same configuration once the allow-list names the front door alone.
const CLOSED: &str =
    r#"{"ServerName":"nas","CorsHosts":["http://nas.local:5055"],"EnableMetrics":false}"#;

/// What the allow-list names is read out of the whole configuration.
#[tokio::test]
async fn the_allow_list_is_read_out_of_the_configuration() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, OPEN),
    ]);

    let held = reader(&fake).cors_hosts().await;

    assert_eq!(held.ok(), Some(vec!["*".to_owned()]));
}

/// The whole configuration goes back with only the allow-list changed, and the write is
/// held to what the server reads back — all of it under the one sign-in.
#[tokio::test]
async fn the_front_door_alone_is_written_and_every_other_field_goes_back_as_it_was() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, OPEN),
        Answer::reply(204, ""),
        Answer::reply(200, CLOSED),
    ]);

    let written = reader(&fake).allow_only(DOOR).await;

    assert!(written.is_ok(), "{written:?}");
    let posted: Vec<serde_json::Value> = fake
        .requests()
        .into_iter()
        .filter(|asked| {
            asked.method == Method::Post && asked.url.ends_with("/System/Configuration")
        })
        .filter_map(|asked| asked.body.and_then(|body| serde_json::from_str(&body).ok()))
        .collect();
    assert_eq!(
        posted,
        vec![serde_json::json!({
            "ServerName": "nas",
            "CorsHosts": [DOOR],
            "EnableMetrics": false
        })]
    );
}

/// A write the server accepted and did not keep is a failure, not a closed list.
#[tokio::test]
async fn a_list_that_reads_back_otherwise_is_a_failure() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, OPEN),
        Answer::reply(204, ""),
        Answer::reply(200, OPEN),
    ]);

    let written = reader(&fake).allow_only(DOOR).await;

    assert!(
        written
            .as_ref()
            .is_err_and(|failure| failure.to_string().contains("reads back as")),
        "{written:?}"
    );
}

/// Nothing and a wildcard are never written: each opens the list to every origin.
#[tokio::test]
async fn an_empty_or_wildcard_origin_is_never_written() {
    for origin in ["", "  ", "*", "http://*.local"] {
        let fake = Fake::in_turn(Vec::new());

        assert!(
            reader(&fake).allow_only(origin).await.is_err(),
            "{origin:?}"
        );
        assert!(fake.requests().is_empty(), "{origin:?} reached the server");
    }
}

/// A configuration that is not an object is refused rather than written over.
#[tokio::test]
async fn a_configuration_that_is_not_an_object_is_not_written_over() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "[]"),
    ]);

    assert!(reader(&fake).allow_only(DOOR).await.is_err());
    assert!(
        !fake
            .requests()
            .iter()
            .any(|asked| asked.method == Method::Post
                && asked.url.ends_with("/System/Configuration")),
        "a configuration that is not one was written over"
    );
}

/// A write the server refuses is reported.
#[tokio::test]
async fn a_write_the_server_refuses_is_reported() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, OPEN),
        Answer::reply(400, ""),
    ]);

    assert!(reader(&fake).allow_only(DOOR).await.is_err());
}
