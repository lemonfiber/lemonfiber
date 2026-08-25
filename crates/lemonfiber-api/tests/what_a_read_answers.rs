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
use lemonfiber_api::events::live::Live;
use lemonfiber_api::events::Streaming;
use lemonfiber_api::guard::{Token, TOKEN_HEADER};
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::read::enveloped;
use lemonfiber_api::router::{routes, Serving};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, QualityAction};
use lemonfiber_core::config::store::REDACTED;
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::ports::{Chance, Idle, Stopped};
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
    holding(engine, stack, Settings::default())
}

/// The same world, told where the settings it reads are kept.
fn holding(engine: Reporting, stack: Source, settings: Settings) -> Ctx {
    Ctx::new(
        Arc::new(Idle),
        Arc::new(engine),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        stack,
        settings,
        Environment::MacOs,
    )
    .with_http(Fake::silent())
}

/// A value built rather than written, so nothing scanning this tree for a
/// committed credential finds a string that reads as one.
fn a_value() -> String {
    ('a'..='j').collect()
}

/// Two settings: one ordinary, and one whose name reads as a credential.
fn kept() -> String {
    format!("LEMONFIBER_USENET=on\nSONARR_API_KEY={}\n", a_value())
}

/// A world whose settings are these, written to a scratch file this test owns.
fn configured(named: &str, contents: &str) -> Ctx {
    let dir = std::env::temp_dir().join(format!("lemonfiber-read-{}-{named}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(".env");
    let _ = std::fs::write(&path, contents);
    holding(
        running(),
        stack(),
        Settings {
            env_file: Some(path),
            ..Settings::default()
        },
    )
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
    let token = Arc::new(Token::mint(&given())?);
    let live = Arc::new(Live::opening(Stopped::at(0).as_ref()));
    let router = routes(
        Serving {
            ctx: Arc::new(ctx),
            token: Arc::clone(&token),
            bound: bound(),
            jobs: Jobs::default(),
        },
        Arc::new(Streaming {
            token,
            bound: bound(),
            live,
        }),
    );

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
async fn what_the_stack_declares_is_the_envelope_the_command_renders() {
    // The gap this read closed: the surface offers to start, stop and switch forms,
    // and until this endpoint existed a caller had to already know their names.
    let expected = as_the_command_renders_it(&world(running(), stack()), Command::Forms).await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/forms").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn the_forms_a_stack_declares_are_carried_in_their_own_envelope() {
    // Written out rather than derived, so a second serialisation could not pass
    // this by agreeing with itself.
    let seen = asked(world(running(), stack()), "/api/forms").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"forms","data":{"forms":[{"#)
            && body.contains(r#""id":"library""#)),
        "every form the stack declares, under the forms kind"
    );
}

#[tokio::test]
async fn naming_a_form_says_what_starting_it_would_come_to() {
    // One endpoint over two commands, because the command line spells the two with
    // one word: naming none lists them, naming some resolves them.
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Preview {
            forms: vec!["library".to_owned()],
        },
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/forms?form=library").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn a_form_that_is_named_is_not_answered_with_the_whole_list() {
    // The mistake the two commands exist to keep apart: a request that named a
    // form and was handed the catalogue would look like it had been answered.
    let seen = asked(world(running(), stack()), "/api/forms?form=library").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"preview""#)),
        "a named form is resolved rather than listed"
    );
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

/// One check, asked for by the identifier its own finding carries.
#[tokio::test]
async fn a_single_check_is_asked_for_the_way_a_finding_names_it() {
    let seen = asked(
        world(running(), stack()),
        "/api/checks?only=environment.engine",
    )
    .await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.contains(r#""check":"environment.engine""#)
            && !body.contains(r#""check":"environment.compose""#)),
        "a read narrowed to one check answers with that check"
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
            "There is no group of checks and no check by that name.".to_owned()
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
async fn the_versions_in_play_are_the_envelope_the_command_renders() {
    // The cheapest read there is: no arguments, and an answer the core already
    // renders for the command line.
    let expected = as_the_command_renders_it(&world(running(), stack()), Command::Version).await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/version").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn the_versions_in_play_are_carried_in_their_own_envelope() {
    // Written out rather than derived, so a second serialisation could not pass
    // this by agreeing with itself.
    let seen = asked(world(running(), stack()), "/api/version").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"version","data":{"binary":"#)),
        "the versions in play, under the version kind"
    );
}

#[tokio::test]
async fn following_one_item_is_the_envelope_the_command_renders() {
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Trace {
            term: "the expanse".to_owned(),
            season: None,
        },
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/trace?term=the+expanse").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn the_term_a_trace_followed_is_the_term_that_was_asked_for() {
    // The whole request is its argument, so a read that dropped it would answer
    // about something else and look like it had answered.
    let seen = asked(world(running(), stack()), "/api/trace?term=the+expanse").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"trace","data":{"#)
            && body.contains(r#""item":"the expanse""#)),
        "the item followed is the one named"
    );
}

#[tokio::test]
async fn a_season_narrows_a_trace_the_way_it_narrows_the_command() {
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Trace {
            term: "the expanse".to_owned(),
            season: Some(2),
        },
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(
            world(running(), stack()),
            "/api/trace?term=the+expanse&season=2"
        )
        .await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn a_trace_that_named_nothing_to_follow_is_refused() {
    // The command line requires the term too. A trace of everything is not a
    // smaller request than a trace of one thing; it is a different one.
    assert_eq!(
        asked(world(running(), stack()), "/api/trace").await,
        Some((
            StatusCode::BAD_REQUEST,
            "What to follow must be named.".to_owned()
        ))
    );
}

#[tokio::test]
async fn a_term_given_and_left_empty_named_nothing_to_follow() {
    assert_eq!(
        asked(world(running(), stack()), "/api/trace?term=").await,
        Some((
            StatusCode::BAD_REQUEST,
            "What to follow must be named.".to_owned()
        ))
    );
}

#[tokio::test]
async fn a_season_that_is_not_a_number_is_refused() {
    assert_eq!(
        asked(
            world(running(), stack()),
            "/api/trace?term=the+expanse&season=latest"
        )
        .await,
        Some((
            StatusCode::BAD_REQUEST,
            "Which season to narrow to must be a number.".to_owned()
        ))
    );
}

#[tokio::test]
async fn what_has_stopped_is_answered_under_its_own_kind() {
    // The landing point for the dashboard's own count of what is stuck, which
    // until this endpoint existed had nowhere on the web to go.
    let seen = asked(world(running(), stack()), "/api/stuck").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"stuck","data":{"items":"#)),
        "the stuck items are answered in the stuck envelope"
    );
}

#[tokio::test]
async fn every_setting_is_the_envelope_the_command_renders() {
    let contents = kept();
    let expected =
        as_the_command_renders_it(&configured("shown-command", &contents), Command::ConfigShow)
            .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(configured("shown-endpoint", &contents), "/api/config").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn a_setting_whose_name_reads_as_a_credential_is_withheld() {
    // The withholding is the core's, so it is in force wherever the settings are
    // read from. This is the endpoint that would have published them.
    let seen = asked(configured("withheld", &kept()), "/api/config").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && !body.contains(&a_value())
            && body.contains(&format!(r#""value":"{REDACTED}","secret":true"#))
            && body.contains(r#""key":"LEMONFIBER_USENET","value":"on","secret":false"#)),
        "the credential is withheld and the setting beside it is not"
    );
}

#[tokio::test]
async fn naming_a_setting_reads_that_one_rather_than_all_of_them() {
    let seen = asked(
        configured("one-setting", &kept()),
        "/api/config?key=LEMONFIBER_USENET",
    )
    .await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.contains(r#""key":"LEMONFIBER_USENET""#)
            && !body.contains("SONARR_API_KEY")),
        "a named setting is the setting reported on"
    );
}

#[tokio::test]
async fn the_quality_in_force_is_the_envelope_the_command_renders() {
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Quality(QualityAction::Show),
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/quality").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn the_quality_in_force_is_carried_in_its_own_envelope() {
    let seen = asked(world(running(), stack()), "/api/quality").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"quality","data":{"#)),
        "the choice in force is answered in the quality envelope"
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
