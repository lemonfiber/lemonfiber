//! What an action is answered with: a name for the work, or the outcome.

use super::acting::*;
use super::{command, nothing, said};
use axum::body::to_bytes;
use axum::http::StatusCode;
use lemonfiber_api::actions::{answering, declined, Answering, Arguments, Refused};
use lemonfiber_core::app::{Command, Setting, Waiting};
use lemonfiber_fixtures::ports::Chance;

#[test]
fn an_action_reaching_the_engine_is_answered_with_a_name_for_the_work() {
    for action in [
        "up",
        "down",
        "switch",
        "restart",
        "pull",
        "seed",
        "adopt",
        "watch",
        "walkthrough",
    ] {
        // Given what each takes: seeding and adopting are whole-stack requests and
        // refuse a form, so naming one would be refused rather than answered.
        let Some(command) = command(action, exactly_what(action)) else {
            unreachable!("every offered action reaches a command");
        };
        assert_eq!(answering(&command), Answering::Later, "{action}");
    }
}

#[test]
fn an_action_confined_to_our_own_files_is_answered_with_its_outcome() {
    let setting = Arguments {
        key: Some("DATA_ROOT".to_owned()),
        value: Some("/srv".to_owned()),
        ..Arguments::default()
    };
    let Some(config) = command("config-set", setting) else {
        unreachable!("a setting with both halves reaches a command");
    };
    let Some(reapply) = command("quality-reapply", nothing()) else {
        unreachable!("reapply takes nothing and reaches a command");
    };
    assert_eq!(answering(&config), Answering::Now);
    assert_eq!(answering(&reapply), Answering::Now);
}

#[test]
fn a_change_asked_to_wait_for_the_downloads_is_answered_with_a_name_for_the_work() {
    // The same change, asked to let what is still coming down finish first. That can
    // last an hour, and a request held open for an hour is a request that has already
    // failed — so this one is the exception among settings changes rather than the
    // rule for them.
    let waiting = Arguments {
        key: Some("LEMONFIBER_TORRENT".to_owned()),
        value: Some("off".to_owned()),
        wait: Waiting::ForTheDownloads,
        ..Arguments::default()
    };
    let Some(dropping) = command("config-set", waiting) else {
        unreachable!("a setting with both halves reaches a command");
    };
    assert_eq!(answering(&dropping), Answering::Later);
}

#[test]
fn a_change_carries_both_words_a_refused_one_is_answered_with() {
    // A change reconfiguration turned away is answered by confirming it or by waiting
    // for what is in flight, and both have to reach the core from a browser — or the
    // refusal a browser is shown is one it has no way to answer.
    let answered = Arguments {
        key: Some("DATA_ROOT".to_owned()),
        value: Some("/srv".to_owned()),
        confirm: true,
        wait: Waiting::ForTheDownloads,
        ..Arguments::default()
    };
    assert_eq!(
        command("config-set", answered),
        Some(Command::ConfigSet(
            Setting::to("DATA_ROOT", "/srv")
                .agreed(true)
                .waiting(Waiting::ForTheDownloads)
        ))
    );
}

#[tokio::test]
async fn a_declined_action_says_why_rather_than_answering_with_a_status_alone() {
    let refusal = Refused::Unknown {
        name: "reticulate".to_owned(),
    };
    let response = declined(&refusal);
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = to_bytes(response.into_body(), usize::MAX).await;
    assert_eq!(body.ok().as_deref(), Some(refusal.said().as_bytes()));
}

#[tokio::test]
async fn a_long_running_action_is_answered_with_a_name_and_left_to_run() {
    let (status, body) = said(Chance::cycling(), "up", r#"{"forms":["tv"]}"#).await;
    assert_eq!(status, StatusCode::ACCEPTED.as_u16());
    assert!(body.contains(r#""kind":"job""#), "{body}");
}

#[tokio::test]
async fn an_immediate_action_is_answered_with_the_outcome_itself() {
    let (status, body) = said(Chance::cycling(), "quality-reapply", "{}").await;
    assert_eq!(status, StatusCode::OK.as_u16());
    // The identical envelope the equivalent command emits, not a shape of its own.
    assert!(body.contains(r#""api_version":1"#), "{body}");
    assert!(body.contains(r#""kind":"quality""#), "{body}");
}

#[tokio::test]
async fn an_immediate_action_that_could_not_be_done_answers_with_the_failure() {
    // Nowhere to record a setting, so the command fails — and it fails in the
    // same envelope every other answer arrives in.
    let asked = r#"{"key":"DATA_ROOT","value":"/srv"}"#;
    let (status, body) = said(Chance::cycling(), "config-set", asked).await;
    // Not 200 with a failure inside it: a client's own idea of a successful call
    // should mean what it says, and which failure it was is the envelope's code.
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
    assert!(body.contains(r#""kind":"error""#), "{body}");
}

#[tokio::test]
async fn a_name_the_route_does_not_offer_is_refused_by_the_route_too() {
    let (status, body) = said(Chance::cycling(), "reticulate", "{}").await;
    assert_eq!(status, StatusCode::NOT_FOUND.as_u16());
    assert!(body.contains("no action named"), "{body}");
}

#[tokio::test]
async fn an_action_missing_an_argument_is_refused_by_the_route_too() {
    let (status, body) = said(Chance::cycling(), "pull", "{}").await;
    assert_eq!(status, StatusCode::BAD_REQUEST.as_u16());
    assert!(body.contains("needs `forms`"), "{body}");
}

#[tokio::test]
async fn agreement_an_action_does_not_take_is_refused_by_the_route_too() {
    let asked = r#"{"forms":["tv"],"confirm":true}"#;
    let (status, body) = said(Chance::cycling(), "down", asked).await;
    assert_eq!(status, StatusCode::BAD_REQUEST.as_u16());
    assert!(body.contains("takes no `confirm`"), "{body}");
}

#[tokio::test]
async fn work_that_cannot_be_named_is_not_started() {
    // A job with no name is work nothing could ever be told about, so it is
    // refused rather than begun and lost.
    let asked = r#"{"forms":["tv"]}"#;
    let (status, body) = said(Chance::exactly(None), "up", asked).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
    assert!(body.contains("randomness"), "{body}");
}
