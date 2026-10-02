//! What an action asked for as a rehearsal answers with.
//!
//! The command line's `--dry-run`, over the web: every action takes `dry_run`, and the
//! core decides what it means exactly as it does for a terminal. A command that reports
//! a rehearsal answers with that report, saying it was one; a command whose effect
//! cannot be known without producing it is refused with the reason it gives. Nothing
//! here decides either — the route only runs the command in a rehearsing copy of its
//! run — which is what this file holds still.
//!
//! Driven from outside the crate, because what a caller can reach is the thing worth
//! holding still.

use std::sync::Arc;
use std::time::Duration;

use axum::body::to_bytes;
use axum::http::{header, StatusCode};
use axum::Extension;
use lemonfiber_api::actions;
use lemonfiber_api::admission::Caller;
use lemonfiber_api::events::live::Live;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::{Jobs, Standing};
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::Settings;
use lemonfiber_fixtures::ports::{Chance, Stopped};
use lemonfiber_fixtures::pulled::Pulled;

/// A run over the stack this repository carries, where nothing a command starts runs.
///
/// The stack is a real one, because a rehearsal of a start reads it to say what would
/// start; its data root is a scratch directory, because a rehearsal that wrote there
/// is the failure being held still and the operator's own is not a place to find out.
fn world() -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("rehearsal").kept();
    lemonfiber_testing::a_context()
        .images(Pulled::holding(Vec::new()))
        .settings(Settings {
            data_root: Some(dir),
            ..Settings::default()
        })
        .build()
        .with_random(Arc::new(Chance::cycling()))
}

/// The route as a run builds it, over the work a test can look into.
fn routed(jobs: Jobs) -> axum::Router {
    let Some(token) = Token::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    actions::routes()
        .with_state(Serving {
            ctx: Arc::new(world()),
            token: Arc::new(token),
            bound: lemonfiber_api::guard::Binding::here(8473),
            admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
            jobs,
            live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
        })
        // The subject the guard puts on every request it admits, for the reason
        // `what_a_disturbing_diagnosis_means` gives.
        .layer(Extension(Caller::Machine))
}

/// What the route answered to one action, as its status and what it said.
async fn said(jobs: Jobs, action: &str, body: &str) -> (u16, String) {
    let request = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/actions/{action}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body.to_owned()));
    let Ok(request) = request else {
        unreachable!("a request built from values that are already headers cannot fail");
    };
    let served = tower::ServiceExt::oneshot(routed(jobs), request).await.ok();
    let Some(response) = served else {
        unreachable!("the router is infallible; its handlers answer rather than fail");
    };
    let status = response.status().as_u16();
    let read = to_bytes(response.into_body(), usize::MAX).await;
    let bytes = read.map(|bytes| bytes.to_vec()).unwrap_or_default();
    (status, String::from_utf8(bytes).unwrap_or_default())
}

/// The name an accepted action's work was given, read off the reply.
fn named(answered: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(answered).unwrap_or_default();
    parsed
        .get("data")
        .and_then(|data| data.get("job"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// What the work came to, once it has had the chance to come to anything.
async fn settled(jobs: &Jobs, job: &str) -> Option<Standing> {
    for _ in 0..200 {
        match jobs.about(job).await.map(|work| work.standing) {
            Some(Standing::Running) | None => {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            settled => return settled,
        }
    }
    jobs.about(job).await.map(|work| work.standing)
}

/// What an action's work came to, as the envelope it settled with.
async fn came_to(action: &str, body: &str) -> (Option<Standing>, String) {
    let jobs = Jobs::default();
    let (status, answered) = said(jobs.clone(), action, body).await;
    assert_eq!(status, StatusCode::ACCEPTED.as_u16(), "{answered}");
    let standing = settled(&jobs, &named(&answered)).await;
    let rendered = match &standing {
        Some(Standing::Done(rendered) | Standing::Failed(rendered, _)) => rendered.clone(),
        _ => String::new(),
    };
    (standing, rendered)
}

#[tokio::test]
async fn a_start_asked_as_a_rehearsal_answers_with_the_report_of_one() {
    let (standing, rendered) = came_to("up", r#"{"forms":["tv"],"dry_run":true}"#).await;
    assert!(matches!(standing, Some(Standing::Done(_))), "{rendered}");
    assert!(rendered.contains(r#""kind":"lifecycle""#), "{rendered}");
    assert!(rendered.contains(r#""rehearsed":true"#), "{rendered}");
}

#[tokio::test]
async fn an_action_whose_command_cannot_be_rehearsed_is_refused_with_its_reason() {
    // A walkthrough is an observation of what the stack actually did with a real
    // item, so the core refuses to rehearse one and says why — over the web as at the
    // command line, because the decision is the core's and this surface has none.
    let (standing, rendered) = came_to("walkthrough", r#"{"item":"Sintel","dry_run":true}"#).await;
    assert!(matches!(standing, Some(Standing::Failed(..))), "{rendered}");
    assert!(rendered.contains(r#""kind":"error""#), "{rendered}");
    assert!(rendered.contains("REHEARSE-"), "{rendered}");
}
