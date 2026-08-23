//! What each read answers with, asked for through the router a request meets.
//!
//! Driven through the assembled router rather than by calling a handler, because
//! what a caller can reach is the thing worth holding still — and because the
//! guard every endpoint sits behind is part of what an endpoint answers.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use lemonfiber_api::guard::{Token, TOKEN_HEADER};
use lemonfiber_api::read::enveloped;
use lemonfiber_api::router::{routes, Serving};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::ports::{Chance, Idle};
use lemonfiber_fixtures::support::Reporting;
use tower::ServiceExt as _;

/// Bytes the test chose, so a token is the same one twice.
///
/// Cycled to whatever width is asked for: a source that answers short mints no
/// token at all, and every request here would then be refused for that instead of
/// answered for what it asked.
fn given() -> Chance {
    Chance::cycling()
}

/// The token every request here carries, read back from the run that minted it.
fn written() -> Option<String> {
    Token::mint(&given()).map(|token| token.as_str().to_owned())
}

/// Built rather than parsed: an address made of numbers cannot fail to be one.
fn bound() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 8471))
}

/// The stack this repository carries, read from disk.
fn stack() -> Source {
    Source::External(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}

/// A stack source pointing nowhere, for the reads that are about a refusal.
fn nowhere() -> Source {
    Source::External(Path::new("/lemonfiber/no/such/stack"))
}

/// The world a command runs against: nothing is spawned and nothing is fetched,
/// so what an endpoint answers is decided by what the engine reports.
fn world(engine: Reporting, stack: Source) -> Ctx {
    Ctx::new(
        Arc::new(Idle),
        Arc::new(engine),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        stack,
        Settings::default(),
        Environment::MacOs,
    )
    .with_http(Fake::silent())
}

/// An engine reporting one healthy service.
fn running() -> Reporting {
    Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy)
}

/// The status and body a request to this path is answered with.
async fn asked(ctx: Ctx, path: &str) -> Option<(StatusCode, String)> {
    let carried = written()?;
    answered(
        ctx,
        path,
        &[("host", "127.0.0.1:8471"), (TOKEN_HEADER, &carried)],
    )
    .await
}

/// The same, for a request that says something else about itself.
async fn answered(ctx: Ctx, path: &str, said: &[(&str, &str)]) -> Option<(StatusCode, String)> {
    let token = Token::mint(&given())?;
    let router = routes(Serving {
        ctx: Arc::new(ctx),
        token: Arc::new(token),
        bound: bound(),
    });

    let mut building = Request::builder().uri(path);
    for (name, value) in said {
        building = building.header(*name, *value);
    }
    let response = router
        .oneshot(building.body(Body::empty()).ok()?)
        .await
        .ok()?;

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.ok()?;
    Some((status, String::from_utf8(body.to_vec()).ok()?))
}

/// The envelope the command line would have rendered for this command.
async fn as_the_command_renders_it(ctx: &Ctx, command: Command) -> Option<String> {
    dispatch(command, ctx)
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json())
}

#[tokio::test]
async fn what_the_stack_is_doing_is_the_envelope_the_command_renders() {
    // The whole of the contract in one assertion: the bytes a browser reads are
    // the bytes a script would have piped, produced by the same three calls.
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Ps { forms: Vec::new() },
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/status").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn what_the_stack_is_doing_is_carried_in_the_envelope_the_contract_states() {
    // Written out rather than derived, so a second serialisation could not pass
    // this by agreeing with itself.
    let seen = asked(world(running(), stack()), "/api/status").await;
    assert!(
        seen.is_some_and(
            |(_, body)| body.starts_with(r#"{"api_version":1,"kind":"status","data":{"forms":[],"#)
        ),
        "the envelope the whole stack is reported in"
    );
}

#[tokio::test]
async fn naming_a_form_narrows_what_the_services_read_reports() {
    let seen = asked(world(running(), stack()), "/api/services?form=library").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.contains(r#""forms":["library"]"#)
            && body.contains(r#""id":"jellyfin""#)),
        "a named form is the form reported on"
    );
}

#[tokio::test]
async fn naming_no_form_reports_on_the_whole_stack() {
    let seen = asked(world(running(), stack()), "/api/services").await;
    assert!(
        seen.is_some_and(
            |(status, body)| status == StatusCode::OK && body.contains(r#""forms":[],"#)
        ),
        "a read that narrows to nothing narrows to nothing"
    );
}

#[tokio::test]
async fn the_checks_answer_under_their_own_kind() {
    let seen = asked(world(running(), stack()), "/api/checks?only=vpn").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"doctor","data":{"#)),
        "a diagnosis is answered in a diagnosis's envelope"
    );
}

#[tokio::test]
async fn a_whole_diagnosis_is_what_a_read_naming_no_group_asks_for() {
    let seen = asked(world(running(), stack()), "/api/checks").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"doctor""#)),
        "naming no group runs every check there is"
    );
}

#[tokio::test]
async fn a_group_of_checks_that_is_not_one_is_not_run() {
    // A name lemonfiber does not know is a mistake to correct, not a request to
    // answer with everything — the judgement the command line makes too.
    assert_eq!(
        asked(world(running(), stack()), "/api/checks?only=nonsense").await,
        Some((
            StatusCode::BAD_REQUEST,
            "There is no group of checks by that name.".to_owned()
        ))
    );
}

#[tokio::test]
async fn the_disk_is_read_through_the_checks_that_are_about_it() {
    let seen = asked(world(running(), stack()), "/api/storage").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"doctor""#)
            && body.contains("storage")),
        "the disk's endpoint is the disk's group of checks"
    );
}

#[tokio::test]
async fn what_the_household_asked_for_is_answered_under_its_own_kind() {
    let seen = asked(world(running(), stack()), "/api/requests").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"household","data":{"#)),
        "the household's requests are answered in the household's envelope"
    );
}

#[tokio::test]
async fn naming_a_member_narrows_what_the_household_read_reports() {
    let seen = asked(world(running(), stack()), "/api/requests?member=ada").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"household""#)),
        "a named member is still the household's envelope"
    );
}

#[tokio::test]
async fn what_the_services_said_arrives_as_one_envelope_a_line() {
    // A stream has no last element to close a document with, so the command line
    // emits an envelope a line and this answers with the same.
    let engine = Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
        .saying_at("sonarr", "2026-01-01T00:00:00Z", "started")
        .saying_at("sonarr", "2026-01-01T00:00:01Z", "importing");

    let seen = asked(world(engine, stack()), "/api/logs?service=sonarr&tail=10").await;
    assert_eq!(
        seen,
        Some((
            StatusCode::OK,
            concat!(
                r#"{"api_version":1,"kind":"log","data":{"service":"sonarr","stream":"stdout","#,
                r#""at":"2026-01-01T00:00:00Z","line":"started"}}"#,
                "\n",
                r#"{"api_version":1,"kind":"log","data":{"service":"sonarr","stream":"stdout","#,
                r#""at":"2026-01-01T00:00:01Z","line":"importing"}}"#,
                "\n",
            )
            .to_owned()
        ))
    );
}

#[tokio::test]
async fn a_service_with_nothing_to_say_answers_with_nothing() {
    // Not "no output": that sentence is for a person, and nobody is reading this.
    let engine = Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy);
    assert_eq!(
        asked(world(engine, stack()), "/api/logs").await,
        Some((StatusCode::OK, String::new()))
    );
}

#[tokio::test]
async fn a_form_narrows_a_log_read_the_way_it_narrows_the_command() {
    let engine = Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy).saying_at(
        "jellyfin",
        "2026-01-01T00:00:00Z",
        "listening",
    );

    let seen = asked(world(engine, stack()), "/api/logs?form=library").await;
    assert!(
        seen.is_some_and(
            |(status, body)| status == StatusCode::OK && body.contains(r#""service":"jellyfin""#)
        ),
        "a form is the services it declares"
    );
}

#[tokio::test]
async fn a_line_count_that_is_not_a_number_is_refused() {
    assert_eq!(
        asked(world(running(), stack()), "/api/logs?tail=plenty").await,
        Some((
            StatusCode::BAD_REQUEST,
            "How many lines to begin with must be a number.".to_owned()
        ))
    );
}

#[tokio::test]
async fn a_command_that_could_not_be_carried_out_answers_with_the_failure() {
    // The envelope a failure gets under `--json`, because a caller that asked for
    // something it could parse asked about the failures most of all.
    let seen = asked(world(running(), nowhere()), "/api/status").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::INTERNAL_SERVER_ERROR
            && body.starts_with(r#"{"api_version":1,"kind":"error","data":{"code":"#)),
        "a failure is an envelope too"
    );
}

#[tokio::test]
async fn a_log_read_that_could_not_be_opened_answers_with_the_failure() {
    let seen = asked(world(Reporting::absent(), stack()), "/api/logs").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::INTERNAL_SERVER_ERROR
            && body.starts_with(r#"{"api_version":1,"kind":"error""#)),
        "an engine that is not there is said in the same envelope"
    );
}

#[tokio::test]
async fn a_request_carrying_no_token_never_reaches_a_read() {
    assert_eq!(
        answered(
            world(running(), stack()),
            "/api/status",
            &[("host", "127.0.0.1:8471")]
        )
        .await,
        Some((
            StatusCode::FORBIDDEN,
            "This request carried no token, or not this run's.".to_owned()
        ))
    );
}

#[tokio::test]
async fn a_path_this_surface_does_not_serve_is_refused_before_it_is_looked_for() {
    // The guard wraps the whole tree, so which paths exist is not something an
    // unauthenticated caller can map by watching the status change.
    let turned_away = answered(
        world(running(), stack()),
        "/api/secrets",
        &[("host", "127.0.0.1:8471")],
    )
    .await;
    assert!(
        turned_away.is_some_and(|(status, _)| status == StatusCode::FORBIDDEN),
        "an unknown path with no token is refused, not reported missing"
    );

    let carrying = asked(world(running(), stack()), "/api/secrets").await;
    assert!(
        carrying.is_some_and(|(status, _)| status == StatusCode::NOT_FOUND),
        "carrying the token, it is simply not there"
    );
}

#[tokio::test]
async fn an_answer_that_could_not_be_rendered_is_not_invented() {
    // Reachable only by being called: these payloads are plain data, so no command
    // can produce one. Answering with an empty document would be worse than saying
    // plainly that there is no answer.
    let response = enveloped(StatusCode::OK, None);
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = to_bytes(response.into_body(), usize::MAX).await;
    assert_eq!(
        body.ok().as_deref(),
        Some("This answer could not be rendered.".as_bytes())
    );
}
