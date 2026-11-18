use axum::body::to_bytes;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use lemonfiber_core::error::codes::play::SERVER_SILENT;
use lemonfiber_core::error::household::Words;
use lemonfiber_core::error::{Problem, Remedy};
use lemonfiber_fixtures::unhoused::unhoused;

use lemonfiber_core::app::Outcome;
use lemonfiber_core::model::{Disturbances, RunHealth, RunReport, ScheduledService, StatusReport};

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
        let sentence = envelope
            .pointer(&format!("/data/{said}"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
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

/// A run that exited non-zero, with the line it wrote last.
fn a_failed_run() -> lemonfiber_core::scheduled::kept::ScheduledRun {
    lemonfiber_core::scheduled::kept::ScheduledRun {
        started: lemonfiber_core::instant::Instant::from_seconds(1_759_400_000),
        ended: lemonfiber_core::instant::Instant::from_seconds(1_759_400_042),
        exit: Some(1),
        log: vec!["sync failed".to_owned()],
    }
}

/// What a status read and a run answer, each carrying a run that wrote a line.
fn logged() -> [Outcome; 2] {
    let last = a_failed_run();
    [
        Outcome::Status(StatusReport {
            forms: Vec::new(),
            active_forms: Vec::new(),
            filtered: Vec::new(),
            condition: lemonfiber_core::docker::Condition::Inactive,
            undeclared: Vec::new(),
            services: Vec::new(),
            disturbs: Disturbances::all(lemonfiber_core::app::PATIENCE),
            waiting: Vec::new(),
            scheduled: vec![ScheduledService {
                id: "configarr".to_owned(),
                name: "Configarr".to_owned(),
                running: false,
                fired: true,
                health: RunHealth::of(Some(&last)),
                last: Some(last.clone()),
                skipped: 0,
            }],
        }),
        Outcome::Ran(RunReport {
            service: "configarr".to_owned(),
            rehearsed: false,
            due: true,
            ran: Some(last),
            skipped: 0,
            held: None,
        }),
    ]
}

/// The lines each run in `outcome` wrote.
fn lines(outcome: &Outcome) -> Vec<String> {
    match outcome {
        Outcome::Status(report) => report
            .scheduled
            .iter()
            .filter_map(|one| one.last.as_ref())
            .flat_map(|run| run.log.clone())
            .collect(),
        Outcome::Ran(report) => report.ran.iter().flat_map(|run| run.log.clone()).collect(),
        _ => Vec::new(),
    }
}

#[test]
fn what_a_run_wrote_reaches_the_operator_alone() {
    let read = lemonfiber_core::keys::Scope::Read;
    let key = Caller::Key(crate::admission::Keyed {
        name: "home-assistant".to_owned(),
        scope: read,
    });
    assert_eq!(Audience::of(&key), Audience::Key);
    for outcome in logged() {
        assert_eq!(
            lines(&Audience::Operator.outcome(outcome.clone())),
            ["sync failed"]
        );
        let keyed = Audience::Key.outcome(outcome.clone());
        assert!(lines(&keyed).is_empty());
        let housed = Audience::Household.outcome(outcome);
        assert!(lines(&housed).is_empty());
    }
}

/// A refusal whose code says how long to wait carries `Retry-After`, and one that says
/// nothing carries none.
#[test]
fn a_refusal_carries_retry_after_only_where_its_code_declares_a_wait() {
    let waits = refused(
        StatusCode::SERVICE_UNAVAILABLE,
        Problem::new(
            lemonfiber_core::error::codes::wish::NOT_CHECKED,
            "Not checked.",
            "Not checked.",
            Remedy::new("Try again."),
        ),
    );
    let silent = refused(StatusCode::BAD_GATEWAY, silent());

    assert_eq!(
        waits
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|seconds| seconds.to_str().ok()),
        Some("5")
    );
    assert!(silent.headers().get(header::RETRY_AFTER).is_none());
}
