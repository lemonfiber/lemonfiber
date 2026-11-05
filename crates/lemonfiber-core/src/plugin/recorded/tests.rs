use super::{records, Of, Recording};

/// A recording of an unauthenticated read.
const WHOLE: &str = r#"{
  "recorded_from": "example.invalid/thing@sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
  "note": "An unauthenticated read. A refusal is the pass.",
  "request": { "method": "GET", "path": "/api/v1/series" },
  "response": { "status": 401 }
}"#;

/// Whether a recording is of the call these terms name, with the request read the
/// way a manifest's is rather than built here.
///
/// `Request` refuses a field it does not know and takes its own defaults, so a
/// literal written beside it could name a shape a manifest cannot. It answers the
/// question rather than handing back the request because a helper that returned one
/// has to say what it does where the text is not a request — and the only answer
/// available there is a line the assertion above has already made unreachable.
fn is_of(recording: &Recording, method: &str, path: &str, accept: Option<&str>) -> bool {
    let quoted = accept.map_or_else(String::new, |accept| format!(r#", "accept": "{accept}""#));
    let text = format!(r#"{{"method": "{method}", "path": "{path}"{quoted}}}"#);
    let read = serde_json::from_str(&text);
    assert!(read.is_ok(), "the request does not read: {read:?}");
    read.is_ok_and(|asked| match &recording.of {
        Of::Request { request, .. } => records(request, &asked),
        Of::Run { .. } => false,
    })
}

#[test]
fn a_recording_of_another_call_is_not_of_this_one() {
    let recording: Result<Recording, _> = serde_json::from_str(WHOLE);
    let asked = |method: &str, path: &str| {
        recording
            .as_ref()
            .is_ok_and(|one| is_of(one, method, path, None))
    };
    assert!(asked("GET", "/api/v1/series"), "the call it records");
    assert!(!asked("POST", "/api/v1/series"), "another method");
    assert!(!asked("GET", "/api/v1/libraries"), "another path");
}

/// What was asked for is part of which call a recording is of.
///
/// Both directions, because only one of them is the mistake anybody would make: a
/// recording taken plainly is not evidence for a request that negotiates, and a
/// recording that negotiated is not evidence for one that did not.
#[test]
fn a_recording_that_asked_for_something_else_is_of_another_call() {
    let plain: Result<Recording, _> = serde_json::from_str(WHOLE);
    let negotiated: Result<Recording, _> = serde_json::from_str(&WHOLE.replace(
        r#""path": "/api/v1/series" }"#,
        r#""path": "/api/v1/series", "accept": "application/json" }"#,
    ));
    assert!(
        negotiated.is_ok(),
        "the recording does not read: {negotiated:?}"
    );
    let asked = |recording: &Result<Recording, serde_json::Error>, accept: Option<&str>| {
        recording
            .as_ref()
            .is_ok_and(|one| is_of(one, "GET", "/api/v1/series", accept))
    };
    let json = Some("application/json");

    assert!(asked(&plain, None));
    assert!(!asked(&plain, json));
    assert!(asked(&negotiated, json));
    assert!(!asked(&negotiated, None));
}
