//! What a proof asks, the headers its request sends, and what it expects back.

use super::{parse, WHOLE};

#[test]
fn reads_what_a_proof_asks_and_why_it_is_worth_asking() {
    let read = parse(WHOLE)
        .and_then(|manifest| manifest.proofs.into_iter().next())
        .map(|proof| {
            (
                proof.id,
                proof.title,
                proof.request.map(|request| request.method),
                proof.fixture,
                proof.why.contains("health probe"),
            )
        });
    assert_eq!(
        read,
        Some((
            "komga.serves".to_owned(),
            "Komga answers on its declared health path".to_owned(),
            Some("GET".to_owned()),
            Some("fixtures/health.json".to_owned()),
            true,
        ))
    );
}

/// A service that answers its application shell for every unimplemented path.
///
/// The shape two of the published plugins already carry, and the reason
/// `json_is_absent` is a flag rather than a list of keys: what has to be said is
/// that the body was not a document at all. A status alone proves nothing against
/// such a service, because the shell comes back `200` whether the API behind it
/// exists or not.
#[test]
fn a_proof_can_say_the_answer_was_not_json_at_all() {
    let text = WHOLE.replace(
            r#"expect  = { status = 200, json = { status = "UP", claimed = true, libraries = 3 } }"#,
            r#"expect  = { status = 200, content_type = "text/html", json_is_absent = true, body_starts_with = "<!DOCTYPE html>" }"#,
        );
    let read = parse(&text)
        .and_then(|manifest| manifest.proofs.into_iter().next())
        .map(|proof| {
            (
                proof.expect.json_is_absent,
                proof.expect.content_type,
                proof.expect.body_starts_with,
                proof.expect.json.is_none(),
            )
        });
    assert_eq!(
        read,
        Some((
            Some(true),
            Some("text/html".to_owned()),
            Some("<!DOCTYPE html>".to_owned()),
            true,
        ))
    );
}

/// A request puts its representation first, then each header it names, and nothing more.
#[test]
fn a_request_sends_its_accept_then_each_header_it_names() {
    let plain = crate::Request {
        method: "GET".to_owned(),
        path: "/".to_owned(),
        accept: None,
        varies: std::collections::BTreeMap::new(),
        headers: std::collections::BTreeMap::new(),
    };
    let named = crate::Request {
        accept: Some("application/json".to_owned()),
        headers: [(
            "Target-URL".to_owned(),
            "http://sonarr:8989/ping".to_owned(),
        )]
        .into(),
        ..plain.clone()
    };

    assert!(plain.sent().is_empty());
    assert_eq!(
        named.sent(),
        [
            ("Accept".to_owned(), "application/json".to_owned()),
            (
                "Target-URL".to_owned(),
                "http://sonarr:8989/ping".to_owned()
            ),
        ]
    );
}
