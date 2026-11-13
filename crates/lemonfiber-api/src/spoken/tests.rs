use axum::body::to_bytes;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use lemonfiber_core::error::codes::play::SERVER_SILENT;
use lemonfiber_core::error::household::{unhoused, Words};
use lemonfiber_core::error::{Problem, Remedy};

use super::{for_audience, refused, Audience, Refused};
use crate::admission::Caller;

/// What the operator is told when the media server did not answer.
fn silent() -> Problem {
    Problem::new(
        SERVER_SILENT,
        "The media server did not answer",
        "Jellyfin at 172.18.0.4:8096 refused the connection.",
        Remedy::new("Check the media server with lemonfiber status"),
    )
    .with_detail("connection refused")
}

/// The body of `response`, as text.
async fn body(response: Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// A member's session and a key acting as one are the household; everybody else is not.
#[test]
fn a_member_is_answered_as_the_household_and_the_operator_as_the_operator() {
    assert_eq!(
        Audience::of(&Caller::Member("a7f3".to_owned())),
        Audience::Household
    );
    assert_eq!(Audience::of(&Caller::Operator), Audience::Operator);
    assert_eq!(Audience::of(&Caller::Machine), Audience::Operator);
}

/// A refusal reaches a member with the household's words and none of the operator's,
/// at the status and with the headers it was answered with.
#[tokio::test]
async fn a_refusal_reaches_a_member_in_the_households_words() {
    let mut answered = refused(StatusCode::BAD_GATEWAY, silent());
    answered
        .headers_mut()
        .insert(header::RETRY_AFTER, HeaderValue::from_static("5"));
    let spoken = for_audience(Audience::Household, answered);
    assert_eq!(spoken.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(
        spoken.headers().get(header::RETRY_AFTER),
        Some(&HeaderValue::from_static("5"))
    );
    let text = body(spoken).await;
    let words = Words::of(SERVER_SILENT);
    assert!(text.contains("PLAY-5"), "{text}");
    assert!(text.contains(words.said), "{text}");
    assert!(text.contains(words.remedy), "{text}");
    for operators in [
        "Jellyfin",
        "172.18.0.4",
        "connection refused",
        "lemonfiber status",
    ] {
        assert!(
            !text.contains(operators),
            "a member was shown {operators:?}: {text}"
        );
    }
    let envelope: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    for said in ["summary", "meaning"] {
        let sentence = envelope["data"][said].as_str().unwrap_or_default();
        assert_eq!(unhoused(sentence), None, "{sentence}");
    }
}

/// The operator is answered with every word the core said.
#[tokio::test]
async fn a_refusal_reaches_the_operator_as_it_was_said() {
    let text = body(for_audience(
        Audience::Operator,
        refused(StatusCode::BAD_GATEWAY, silent()),
    ))
    .await;
    assert!(text.contains("Jellyfin at 172.18.0.4:8096"), "{text}");
    assert!(text.contains("connection refused"), "{text}");
}

/// A response carrying no refusal is left as it is, whoever it reaches.
#[tokio::test]
async fn an_answer_that_is_no_refusal_is_left_as_it_is() {
    let answered = crate::serve::answered("{\"kind\":\"held\"}".to_owned());
    let spoken = for_audience(Audience::Household, answered);
    assert!(spoken.extensions().get::<Refused>().is_none());
    assert_eq!(body(spoken).await, "{\"kind\":\"held\"}");
}
