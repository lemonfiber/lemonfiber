use std::collections::BTreeMap;

use lemonfiber_plugin::{Capture, Condition, Origin};

use super::{captured, ended, holds};
use crate::plugin::Answer;

/// An answer with this body and these headers.
fn answer(body: &str, headers: &[(&str, &str)]) -> Answer {
    Answer {
        status: 200,
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        json: serde_json::from_str(body).ok(),
        body_starts_with: Some(body.to_owned()),
    }
}

/// A capture of `from` named `name`.
fn capture(name: &str, from: &str) -> Capture {
    Capture {
        name: name.to_owned(),
        from: from.to_owned(),
        origin: Origin::StackService,
    }
}

#[test]
fn a_capture_reads_a_key_a_pointer_or_a_header() {
    let read = captured(
        &[
            capture("token", "token"),
            capture("id", "/MediaContainer/Setting/[id=PlexOnlineToken]/value"),
            capture("count", "count"),
            capture("server", "header.X-Plex-Server"),
        ],
        &answer(
            r#"{"token":"abc","count":3,"MediaContainer":{"Setting":[{"id":"Other","value":"no"},{"id":"PlexOnlineToken","value":"xyz"}]}}"#,
            &[("x-plex-server", "box")],
        ),
    );
    assert_eq!(
        read,
        Ok(BTreeMap::from([
            ("token".to_owned(), "abc".to_owned()),
            ("id".to_owned(), "xyz".to_owned()),
            ("count".to_owned(), "3".to_owned()),
            ("server".to_owned(), "box".to_owned()),
        ]))
    );
}

#[test]
fn a_capture_reading_nothing_says_where_it_looked() {
    let body = captured(&[capture("token", "token")], &answer("{}", &[]));
    assert!(body.is_err_and(|why| why.contains("token")));
    let header = captured(&[capture("token", "header.X-Token")], &answer("{}", &[]));
    assert!(header.is_err_and(|why| why.contains("X-Token header")));
}

/// A guard holds on an earlier status or value, and on nothing it was not shaped as.
#[test]
fn a_guard_holds_on_what_came_before() {
    let answered = BTreeMap::from([("in".to_owned(), 200)]);
    let values = BTreeMap::from([("word".to_owned(), "yes".to_owned())]);
    let on =
        |step: Option<&str>, status: Option<u16>, value: Option<&str>, equals: Option<&str>| {
            holds(
                &Condition {
                    step: step.map(str::to_owned),
                    status,
                    value: value.map(str::to_owned),
                    equals: equals.map(str::to_owned),
                },
                &answered,
                &values,
            )
        };
    assert!(on(Some("in"), Some(200), None, None));
    assert!(!on(Some("in"), Some(404), None, None));
    assert!(!on(Some("never"), Some(200), None, None));
    assert!(on(None, None, Some("word"), Some("yes")));
    assert!(!on(None, None, Some("word"), Some("no")));
    assert!(!on(None, None, None, None));
}

/// A retry ends on the status its last answer carried, or on a value.
#[test]
fn a_retry_ends_on_a_status_or_a_value() {
    let values = BTreeMap::from([("ready".to_owned(), "true".to_owned())]);
    let status = Condition {
        status: Some(200),
        ..Condition::default()
    };
    assert!(ended(&status, 200, &values));
    assert!(!ended(&status, 503, &values));
    let value = Condition {
        value: Some("ready".to_owned()),
        equals: Some("true".to_owned()),
        ..Condition::default()
    };
    assert!(ended(&value, 503, &values));
    assert!(!ended(&value, 200, &BTreeMap::new()));
}
