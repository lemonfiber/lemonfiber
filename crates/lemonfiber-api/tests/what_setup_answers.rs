//! What setup answers when it is walked a request at a time.
//!
//! The load-bearing property is not that any one step works. It is that a browser
//! can complete first-run setup without this surface deciding anything: every
//! request becomes one of the core's own commands, the answers between requests
//! live in the file a terminal run reads too, and what comes back never repeats
//! what was entered.
//!
//! Driven through the router rather than by calling a handler, because what a
//! caller can reach is the thing worth holding still.

use std::path::Path;
use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use lemonfiber_api::events::live::Live;
use lemonfiber_api::events::Streaming;
use lemonfiber_api::guard::{Binding, Token, TOKEN_HEADER};
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::ports::{Chance, Idle, Stopped};
use lemonfiber_fixtures::support::Reporting;
use tower::ServiceExt as _;

/// Serving this machine and nowhere else, at the port these tests name.
fn bound() -> Binding {
    Binding::here(8471)
}

/// A scratch layout unique to this process and case, cleared first.
fn scratch(name: &str) -> Paths {
    let dir = std::env::temp_dir().join(format!(
        "lemonfiber-api-setup-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    Paths::rooted(&dir.join("config"), &dir.join("data"))
}

/// The world these requests run against: files in a scratch layout, a stack
/// already on disk so applying materialises nothing, and nothing fetched.
fn world(paths: &Paths) -> Ctx {
    Ctx::new(
        Arc::new(Idle),
        Arc::new(Reporting::absent()),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        Source::External(Path::new("/lemonfiber/no/such/stack")),
        Settings {
            env_file: Some(paths.env_file()),
            stack_dir: Some(paths.stack()),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(Fake::silent())
}

/// What this run serves, over the world a test chose.
fn serving(paths: &Paths) -> Option<Serving> {
    Some(Serving {
        ctx: Arc::new(world(paths)),
        token: Arc::new(Token::mint(&Chance::cycling())?),
        bound: bound(),
        admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
        jobs: Jobs::default(),
        live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
    })
}

/// A run with nowhere to keep configuration at all, which is not the caller's
/// mistake and is not answered as one.
fn homeless() -> Option<axum::Router> {
    let serving = Serving {
        ctx: Arc::new(
            Ctx::new(
                Arc::new(Idle),
                Arc::new(Reporting::absent()),
                Arc::new(lemonfiber_core::adapters::System),
                Arc::new(lemonfiber_core::adapters::Disk),
                Source::External(Path::new("/lemonfiber/no/such/stack")),
                Settings::default(),
                Environment::MacOs,
            )
            .with_http(Fake::silent()),
        ),
        token: Arc::new(Token::mint(&Chance::cycling())?),
        bound: bound(),
        admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
        jobs: Jobs::default(),
        live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
    };
    Some(lemonfiber_api::setup::routes().with_state(serving))
}

/// The setup routes alone, over that world.
///
/// Stated here rather than assembled with the rest of the surface: what setup
/// *answers* is this file's business, and that a request reaches it only with a
/// token is the router's, proven once below.
fn routed(paths: &Paths) -> Option<axum::Router> {
    Some(lemonfiber_api::setup::routes().with_state(serving(paths)?))
}

/// One request to a setup endpoint, and what it was answered with.
async fn asked(paths: &Paths, method: &str, path: &str, body: &str) -> Option<(u16, String)> {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .ok()?;
    let response = routed(paths)?.oneshot(request).await.ok()?;
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.ok()?;
    Some((
        status,
        String::from_utf8(bytes.to_vec()).unwrap_or_default(),
    ))
}

/// Where setup stands, as a read of it says.
async fn standing(paths: &Paths) -> Option<(u16, String)> {
    asked(paths, "GET", "/api/setup", "").await
}

/// One answer submitted.
async fn answer(paths: &Paths, body: &str) -> Option<(u16, String)> {
    asked(paths, "POST", "/api/setup/answer", body).await
}

/// A value that must not be printed back, assembled rather than written out so no
/// credential sits in this source.
fn withheld_value(word: &str) -> String {
    [word, "not", "real"].join("-")
}

/// Every answer this platform asks for, as a browser would send them.
fn every_answer(root: &Path) -> Vec<String> {
    vec![
        r#"{"protocols":{"usenet":true,"torrent":true}}"#.to_owned(),
        r#"{"vpn":"carrying"}"#.to_owned(),
        format!(
            r#"{{"data-location":{}}}"#,
            serde_json::to_string(root).unwrap_or_default()
        ),
        r#"{"credentials":null}"#.to_owned(),
        r#"{"provider":null}"#.to_owned(),
        r#"{"library":"none"}"#.to_owned(),
        r#"{"household":false}"#.to_owned(),
        r#"{"notifications":"problems-only"}"#.to_owned(),
        r#"{"autostart":false}"#.to_owned(),
    ]
}

#[tokio::test]
async fn a_fresh_machine_is_answered_with_where_setup_stands() {
    let paths = scratch("fresh");
    let said = standing(&paths).await;

    assert_eq!(said.as_ref().map(|(status, _)| *status), Some(200));
    let body = said.map(|(_, body)| body).unwrap_or_default();
    // The same envelope the command line emits, not a shape of its own.
    assert!(body.contains(r#""api_version":1"#), "{body}");
    assert!(body.contains(r#""kind":"wizard""#), "{body}");
    assert!(body.contains(r#""offered":true"#), "{body}");
    assert!(body.contains(r#""at":"welcome""#), "{body}");
}

#[tokio::test]
async fn an_answer_is_taken_and_the_walk_moves_on() {
    let paths = scratch("answered");
    let said = answer(&paths, r#"{"protocols":{"usenet":true,"torrent":true}}"#).await;

    assert_eq!(said.as_ref().map(|(status, _)| *status), Some(200));
    let body = said.map(|(_, body)| body).unwrap_or_default();
    assert!(body.contains(r#""at":"preflight""#), "{body}");
    assert!(
        !body.contains(r#""protocols""#),
        "answered, so not outstanding: {body}"
    );

    // And a second request picks it up, which is the whole point: nothing is held
    // between them.
    let again = standing(&paths)
        .await
        .map(|(_, body)| body)
        .unwrap_or_default();
    assert!(!again.contains(r#""protocols""#), "{again}");
}

#[tokio::test]
async fn the_steps_that_only_inform_are_passed_and_stepped_back_through() {
    let paths = scratch("walking");
    let onward = asked(&paths, "POST", "/api/setup/next", "")
        .await
        .map(|(_, body)| body)
        .unwrap_or_default();
    assert!(onward.contains(r#""at":"preflight""#), "{onward}");

    let back = asked(&paths, "POST", "/api/setup/back", "")
        .await
        .map(|(_, body)| body)
        .unwrap_or_default();
    assert!(back.contains(r#""at":"welcome""#), "{back}");
}

#[tokio::test]
async fn an_answer_this_platform_does_not_offer_is_refused_in_the_same_envelope() {
    let paths = scratch("rejected");
    // Ownership is mapped away on this platform, so a container user would buy
    // nothing and the wizard refuses to record one.
    let said = answer(&paths, r#"{"service-user":[1000,1000]}"#).await;

    // Not a bare status: which refusal it was is the envelope's code, and a caller
    // that asked for something it could parse asked about the refusals most of all.
    assert_eq!(said.as_ref().map(|(status, _)| *status), Some(400));
    let body = said.map(|(_, body)| body).unwrap_or_default();
    assert!(body.contains(r#""kind":"error""#), "{body}");
    assert!(body.contains(r#""code":"SETUP-5""#), "{body}");
    assert!(body.contains(r#""remedies""#), "and what to do: {body}");
}

#[tokio::test]
async fn applying_before_every_question_is_answered_is_refused_in_the_same_envelope() {
    let paths = scratch("early");
    let said = asked(&paths, "POST", "/api/setup/apply", "").await;

    assert_eq!(said.as_ref().map(|(status, _)| *status), Some(400));
    let body = said.map(|(_, body)| body).unwrap_or_default();
    assert!(body.contains(r#""code":"SETUP-1""#), "{body}");
}

#[tokio::test]
async fn a_body_that_is_not_one_of_setups_answers_is_said_plainly() {
    let paths = scratch("unreadable");
    let said = answer(&paths, r#"{"reticulate":true}"#).await;

    assert_eq!(said.as_ref().map(|(status, _)| *status), Some(400));
    let body = said.map(|(_, body)| body).unwrap_or_default();
    assert!(body.contains("not one of setup's answers"), "{body}");
    // What arrived is not quoted back: an answer carries a credential, and a
    // message repeating the body would carry it wherever the message goes.
    assert!(!body.contains("reticulate"), "{body}");
}

#[tokio::test]
async fn a_setup_walked_over_requests_writes_the_configuration() {
    let paths = scratch("applied");
    let root = paths.data_dir().join("media");
    for body in every_answer(&root) {
        assert_eq!(
            answer(&paths, &body).await.map(|(status, _)| status),
            Some(200),
            "{body}"
        );
    }

    let said = asked(&paths, "POST", "/api/setup/apply", "").await;
    assert_eq!(said.as_ref().map(|(status, _)| *status), Some(200));
    let body = said.map(|(_, body)| body).unwrap_or_default();
    assert!(body.contains(r#""phase":"applied""#), "{body}");
    assert!(body.contains(r#""offered":false"#), "{body}");
    assert!(paths.env_file().exists(), "the settings landed");
    assert!(root.is_dir(), "and so did the library's home");
}

#[tokio::test]
async fn what_was_entered_never_comes_back() {
    let paths = scratch("withholding");
    let key = withheld_value("indexer");
    assert_eq!(
        answer(&paths, r#"{"protocols":{"usenet":true,"torrent":true}}"#)
            .await
            .map(|(status, _)| status),
        Some(200)
    );
    let submitted =
        format!(r#"{{"credentials":{{"url":"http://indexer.invalid/api","key":"{key}"}}}}"#);
    let said = answer(&paths, &submitted).await.map(|(_, body)| body);

    let body = said.unwrap_or_default();
    assert!(!body.contains(&key), "the key came back: {body}");
    assert!(
        body.contains("(set, not shown)"),
        "and is marked present: {body}"
    );
    // Nor on a later read of where things stand.
    let later = standing(&paths)
        .await
        .map(|(_, body)| body)
        .unwrap_or_default();
    assert!(!later.contains(&key), "{later}");
    // A caller cannot assert a credential was proven by saying it was: what is
    // recorded is what the live test established, and nothing answered here.
    assert!(
        later.contains(r#"{"key":"INDEXER_VALIDATED","value":"off""#),
        "{later}"
    );
    // And the browser is told why rather than left to guess, in the terms the
    // remedy differs by: nothing answered, so nothing can be concluded.
    assert!(
        body.contains(r#""proof":{"outcome":"unreachable""#),
        "{body}"
    );
}

#[tokio::test]
async fn an_answer_about_this_machine_has_no_service_to_prove_it_against() {
    let paths = scratch("nothing-to-prove");
    let body = answer(&paths, r#"{"protocols":{"usenet":true,"torrent":true}}"#)
        .await
        .map(|(_, body)| body)
        .unwrap_or_default();

    assert!(body.contains(r#""proof":null"#), "{body}");
}

/// What an interrupted apply leaves in a scratch layout: half-written settings,
/// the marker saying the writing had begun, and the record of what it wrote.
fn interrupted(paths: &Paths) {
    let _ = std::fs::create_dir_all(paths.config_dir());
    assert!(std::fs::write(paths.env_file(), "DATA_ROOT=/srv\n").is_ok());
    assert!(std::fs::write(
        paths.setup_progress(),
        r#"{"at":"review","answers":{},"phase":"applying"}"#
    )
    .is_ok());
    assert!(std::fs::write(
        paths.journal(),
        r#"{"at":"1","operation":"setup","target":".env","kind":{"action":"set","key":"DATA_ROOT","previous":null,"current":"/srv"}}"#
    )
    .is_ok());
}

#[tokio::test]
async fn an_apply_that_stopped_part_way_names_what_it_wrote_before_a_way_out_is_chosen() {
    let paths = scratch("half-written");
    interrupted(&paths);

    let body = standing(&paths)
        .await
        .map(|(_, body)| body)
        .unwrap_or_default();

    assert!(body.contains(r#""phase":"applying""#), "{body}");
    assert!(
        body.contains(r#""written":["the setting DATA_ROOT"]"#),
        "a choice made without seeing this is a choice made blind: {body}"
    );
}

#[tokio::test]
async fn a_way_out_of_an_interrupted_apply_is_taken_and_leaves_nothing_of_it() {
    let paths = scratch("started-over");
    interrupted(&paths);

    let said = asked(
        &paths,
        "POST",
        "/api/setup/recover",
        r#"{"choice":"start-over"}"#,
    )
    .await;

    assert_eq!(said.as_ref().map(|(status, _)| *status), Some(200));
    let body = said.map(|(_, body)| body).unwrap_or_default();
    assert!(body.contains(r#""at":"welcome""#), "{body}");
    assert!(!paths.setup_progress().exists(), "the answers are gone");
    assert!(
        !paths.journal().exists(),
        "and so is the record of the apply"
    );
}

#[tokio::test]
async fn a_way_out_of_an_apply_that_never_stopped_is_refused_in_the_same_envelope() {
    let paths = scratch("nothing-to-recover");
    let said = asked(
        &paths,
        "POST",
        "/api/setup/recover",
        r#"{"choice":"roll-back"}"#,
    )
    .await;

    assert_eq!(said.as_ref().map(|(status, _)| *status), Some(400));
    let body = said.map(|(_, body)| body).unwrap_or_default();
    assert!(body.contains(r#""code":"SETUP-8""#), "{body}");
}

#[tokio::test]
async fn a_body_that_is_not_a_way_out_is_said_plainly() {
    let paths = scratch("not-a-way-out");
    let said = asked(
        &paths,
        "POST",
        "/api/setup/recover",
        r#"{"choice":"reticulate"}"#,
    )
    .await;

    assert_eq!(said.as_ref().map(|(status, _)| *status), Some(400));
    let body = said.map(|(_, body)| body).unwrap_or_default();
    assert!(body.contains("interrupted apply"), "{body}");
    assert!(!body.contains("reticulate"), "{body}");
}

#[tokio::test]
async fn a_machine_already_set_up_is_told_so_rather_than_asked_again() {
    let paths = scratch("configured");
    let _ = std::fs::create_dir_all(paths.config_dir());
    assert!(std::fs::write(paths.env_file(), "DATA_ROOT=/srv\n").is_ok());

    let said = answer(&paths, r#"{"household":false}"#).await;
    assert_eq!(said.as_ref().map(|(status, _)| *status), Some(400));
    let body = said.map(|(_, body)| body).unwrap_or_default();
    assert!(body.contains(r#""code":"SETUP-7""#), "{body}");

    // Asking is still answered: whether setup is on offer is exactly what a
    // browser asks this to decide whether to draw the wizard at all.
    let read = standing(&paths).await;
    assert_eq!(read.as_ref().map(|(status, _)| *status), Some(200));
    assert!(
        read.map(|(_, body)| body)
            .unwrap_or_default()
            .contains(r#""offered":false"#),
        "an already-configured machine says so rather than refusing to say"
    );
}

#[tokio::test]
async fn every_endpoint_setup_offers_reaches_the_core() {
    // The whole guarantee, in one sweep: a path this surface serves that reached
    // no command would be a step only a browser has. Each answers with an
    // envelope — the wizard's, or the error one a command raised — and none is
    // answered by the router having nothing there.
    let paths = scratch("sweep");
    let endpoints = [
        ("GET", "/api/setup", ""),
        ("POST", "/api/setup/answer", r#"{"household":false}"#),
        ("POST", "/api/setup/next", ""),
        ("POST", "/api/setup/back", ""),
        ("POST", "/api/setup/apply", ""),
        ("POST", "/api/setup/recover", r#"{"choice":"resume"}"#),
    ];
    for (method, path, body) in endpoints {
        let said = asked(&paths, method, path, body).await;
        let (status, answered) = said.unwrap_or((0, String::new()));
        assert_ne!(status, StatusCode::NOT_FOUND.as_u16(), "{path}");
        assert_ne!(status, StatusCode::METHOD_NOT_ALLOWED.as_u16(), "{path}");
        assert!(
            answered.contains(r#""api_version":1"#),
            "{path} answered outside the envelope: {answered}"
        );
    }
}

#[tokio::test]
async fn no_setup_endpoint_is_reachable_without_this_run_s_token() {
    // Through the assembled surface rather than the routes alone: admission is
    // asked once above the whole tree, and these arrived after it was written.
    let paths = scratch("guarded");
    let Some(serving) = serving(&paths) else {
        unreachable!("cycling letters always supply bytes")
    };
    let streaming = Arc::new(Streaming {
        admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
        token: Arc::clone(&serving.token),
        bound: bound(),
        live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
    });
    let surface = lemonfiber_api::router::routes(serving, streaming);

    for (method, path) in [
        ("GET", "/api/setup"),
        ("POST", "/api/setup/answer"),
        ("POST", "/api/setup/next"),
        ("POST", "/api/setup/back"),
        ("POST", "/api/setup/apply"),
        ("POST", "/api/setup/recover"),
    ] {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("host", "127.0.0.1:8471")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .ok();
        let status = match request {
            Some(request) => surface
                .clone()
                .oneshot(request)
                .await
                .ok()
                .map(|response| response.status()),
            None => None,
        };
        assert_eq!(status, Some(StatusCode::FORBIDDEN), "{path}");
    }
}

#[tokio::test]
async fn a_token_from_this_run_is_admitted() {
    // The other half of the guard: refusing everything would pass the test above.
    let paths = scratch("admitted");
    let Some(serving) = serving(&paths) else {
        unreachable!("cycling letters always supply bytes")
    };
    let carried = serving.token.as_str().to_owned();
    let streaming = Arc::new(Streaming {
        admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
        token: Arc::clone(&serving.token),
        bound: bound(),
        live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
    });
    let surface = lemonfiber_api::router::routes(serving, streaming);
    let request = Request::builder()
        .method("GET")
        .uri("/api/setup")
        .header("host", "127.0.0.1:8471")
        .header(TOKEN_HEADER, carried)
        .body(Body::empty())
        .ok();
    let status = match request {
        Some(request) => surface.oneshot(request).await.ok().map(|r| r.status()),
        None => None,
    };
    assert_eq!(status, Some(StatusCode::OK));
}

#[tokio::test]
async fn a_machine_that_cannot_keep_configuration_is_not_the_caller_s_mistake() {
    // The other side of the status: a browser told 400 would go looking for
    // something wrong with what it sent, and there is nothing wrong with it.
    let request = Request::builder()
        .method("GET")
        .uri("/api/setup")
        .body(Body::empty())
        .ok();
    let answered = match (homeless(), request) {
        (Some(router), Some(request)) => router.oneshot(request).await.ok(),
        _ => None,
    };
    let status = answered.as_ref().map(axum::response::Response::status);
    assert_eq!(status, Some(StatusCode::INTERNAL_SERVER_ERROR));

    let body = match answered {
        Some(response) => to_bytes(response.into_body(), usize::MAX).await.ok(),
        None => None,
    };
    let said = body
        .map(|bytes| String::from_utf8(bytes.to_vec()).unwrap_or_default())
        .unwrap_or_default();
    assert!(said.contains(r#""kind":"error""#), "{said}");
}
