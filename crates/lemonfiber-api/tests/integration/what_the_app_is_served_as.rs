//! What a browser is told the app's files are, and what it may do with them.
//!
//! Which file a path names is settled in the core and proven there. What is held
//! still here is everything a browser acts on: the type, which decides whether a
//! stylesheet is applied or downloaded; the caching, which decides whether an old
//! app can outlive the binary that served it; and the two headers that stop a page
//! holding a token from being sniffed or framed.
//!
//! The app served here is the same fixture the core reads, because it stands in
//! for the same absent submodule. Two fixtures for one missing thing would be two
//! things to keep in step.

use std::path::Path;

use axum::body::to_bytes;
use axum::http::{header, StatusCode};
use lemonfiber_api::frontend::{absent, content_type, missing, page, routes, served, unanswered};
use lemonfiber_core::frontend::Source;

/// The app a build would embed, read from disk instead.
fn app() -> Source {
    Source::External(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../lemonfiber-core/tests/fixtures/frontend"
    )))
}

/// What a header on a response says, as text.
fn said(response: &axum::http::Response<axum::body::Body>, name: header::HeaderName) -> String {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn each_kind_of_file_is_named_as_the_kind_it_is() {
    for (file, kind) in [
        ("index.html", "text/html; charset=utf-8"),
        ("app.css", "text/css; charset=utf-8"),
        ("app.js", "text/javascript; charset=utf-8"),
        ("app.mjs", "text/javascript; charset=utf-8"),
        ("app.js.map", "application/json"),
        ("site.webmanifest", "application/manifest+json"),
        ("logo.svg", "image/svg+xml"),
        ("shot.png", "image/png"),
        ("shot.jpg", "image/jpeg"),
        ("shot.jpeg", "image/jpeg"),
        ("shot.webp", "image/webp"),
        ("shot.avif", "image/avif"),
        ("favicon.ico", "image/x-icon"),
        ("body.woff2", "font/woff2"),
        ("body.woff", "font/woff"),
        ("body.ttf", "font/ttf"),
        ("engine.wasm", "application/wasm"),
        ("notes.txt", "text/plain; charset=utf-8"),
    ] {
        assert_eq!(content_type(Path::new(file)), kind, "{file}");
    }
}

#[test]
fn a_kind_this_surface_does_not_know_is_handed_over_rather_than_guessed_at() {
    // A wrong type is acted on; an unknown one is downloaded, which is the safe
    // direction for a file this surface did not expect to be serving.
    assert_eq!(
        content_type(Path::new("something.zzz")),
        "application/octet-stream"
    );
    assert_eq!(
        content_type(Path::new("no-extension")),
        "application/octet-stream"
    );
}

#[test]
fn the_kind_is_read_however_the_name_was_capitalised() {
    assert_eq!(
        content_type(Path::new("INDEX.HTML")),
        "text/html; charset=utf-8"
    );
}

#[tokio::test]
async fn the_page_is_the_app_itself() {
    let response = page(Some(app()), "/");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        said(&response, header::CONTENT_TYPE),
        "text/html; charset=utf-8"
    );

    let body = to_bytes(response.into_body(), usize::MAX).await;
    let text = String::from_utf8(body.map(|bytes| bytes.to_vec()).unwrap_or_default());
    assert!(text.unwrap_or_default().contains("<!doctype html>"));
}

#[tokio::test]
async fn a_file_the_app_holds_is_served_as_what_it_is() {
    let response = page(Some(app()), "/assets/app.js");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        said(&response, header::CONTENT_TYPE),
        "text/javascript; charset=utf-8"
    );

    let body = to_bytes(response.into_body(), usize::MAX).await;
    let text = String::from_utf8(body.map(|bytes| bytes.to_vec()).unwrap_or_default());
    assert!(text.unwrap_or_default().contains("the app"));
}

#[test]
fn a_file_the_app_does_not_hold_is_absent_rather_than_the_page() {
    let response = page(Some(app()), "/assets/nothing.css");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        said(&response, header::CONTENT_TYPE),
        "text/plain; charset=utf-8"
    );
}

#[test]
fn a_path_climbing_out_of_the_app_reaches_nothing() {
    for asked in ["/../Cargo.toml", "/assets/../../Cargo.toml"] {
        assert_eq!(
            page(Some(app()), asked).status(),
            StatusCode::NOT_FOUND,
            "{asked}"
        );
    }
}

#[tokio::test]
async fn a_build_carrying_no_app_says_so_rather_than_answering_with_nothing() {
    // Two different faults: a path nobody wrote, and a binary built without the
    // surface it is being asked for.
    for response in [page(None, "/"), absent()] {
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = to_bytes(response.into_body(), usize::MAX).await;
        let text = String::from_utf8(body.map(|bytes| bytes.to_vec()).unwrap_or_default());
        assert!(text.unwrap_or_default().contains("carries no web app"));
    }
}

#[tokio::test]
async fn a_directory_that_is_not_an_app_is_the_same_as_no_app_at_all() {
    let response = page(
        Some(Source::External(Path::new("/lemonfiber/no/such/app"))),
        "/assets/app.js",
    );
    let body = to_bytes(response.into_body(), usize::MAX).await;
    let text = String::from_utf8(body.map(|bytes| bytes.to_vec()).unwrap_or_default());
    assert!(text.unwrap_or_default().contains("carries no web app"));
}

#[tokio::test]
async fn the_three_absences_do_not_say_the_same_thing() {
    // A file the app does not hold, a build with no app, and a path the
    // endpoints own: three different faults, and each is worth telling apart.
    let mut said = Vec::new();
    for response in [missing(), absent(), unanswered()] {
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let read = to_bytes(response.into_body(), usize::MAX).await;
        said.push(String::from_utf8(read.map(|bytes| bytes.to_vec()).unwrap_or_default()).ok());
    }
    said.sort();
    said.dedup();
    assert_eq!(said.len(), 3, "each says its own thing");
}

#[test]
fn nothing_the_app_is_served_with_may_be_kept() {
    // A browser holding yesterday's app is a browser drawing today's fields
    // under yesterday's meanings, and these bytes come out of memory anyway.
    for asked in ["/", "/assets/app.css", "/assets/nothing.css"] {
        assert_eq!(
            said(&page(Some(app()), asked), header::CACHE_CONTROL),
            "no-store",
            "{asked}"
        );
    }
}

#[test]
fn every_file_is_served_as_the_type_it_was_given_and_no_other() {
    for asked in ["/", "/assets/app.css", "/assets/nothing.css"] {
        assert_eq!(
            said(&page(Some(app()), asked), header::X_CONTENT_TYPE_OPTIONS),
            "nosniff",
            "{asked}"
        );
    }
}

#[test]
fn the_page_says_what_it_may_load_and_refuses_to_be_framed() {
    // It holds the token an operator was given, so what may surround it and what
    // it may reach are stated rather than left open.
    let policy = said(&page(Some(app()), "/"), header::CONTENT_SECURITY_POLICY);
    assert!(policy.contains("default-src 'self'"), "{policy}");
    assert!(policy.contains("frame-ancestors 'none'"), "{policy}");
    assert!(policy.contains("base-uri 'none'"), "{policy}");
}

#[tokio::test]
async fn one_file_served_on_its_own_carries_the_same_guarantees() {
    let Some(asset) = app().asset("/assets/app.css") else {
        unreachable!("the fixture holds a stylesheet");
    };
    let response = served(&asset);
    assert_eq!(
        said(&response, header::CONTENT_TYPE),
        "text/css; charset=utf-8"
    );
    assert_eq!(said(&response, header::CACHE_CONTROL), "no-store");

    let body = to_bytes(response.into_body(), usize::MAX).await;
    let text = String::from_utf8(body.map(|bytes| bytes.to_vec()).unwrap_or_default());
    assert!(text.unwrap_or_default().contains("margin"));
}

// ── The routes themselves, driven without a socket ────────────────────────────

/// What asking the app's routes for a path answers with.
async fn asked_for(app: Option<Source>, path: &str) -> (u16, String) {
    let request = axum::http::Request::builder()
        .uri(path)
        .body(axum::body::Body::empty());
    let Ok(request) = request else {
        unreachable!("a request built from a path that is already one cannot fail");
    };
    let served = tower::ServiceExt::oneshot(routes(app), request).await.ok();
    let Some(response) = served else {
        unreachable!("the router is infallible; its handlers answer rather than fail");
    };
    let status = response.status().as_u16();
    let read = to_bytes(response.into_body(), usize::MAX).await;
    let bytes = read.map(|bytes| bytes.to_vec()).unwrap_or_default();
    (status, String::from_utf8(bytes).unwrap_or_default())
}

#[tokio::test]
async fn every_path_the_endpoints_do_not_answer_reaches_the_app() {
    // One fallback rather than a route per file: the app's own router reads the
    // path once the page is loaded.
    for path in ["/", "/services/sonarr", "/settings/quality"] {
        let (status, body) = asked_for(Some(app()), path).await;
        assert_eq!(status, 200, "{path}");
        assert!(body.contains("<!doctype html>"), "{path}: {body}");
    }
}

#[tokio::test]
async fn a_file_the_routes_are_asked_for_arrives_as_that_file() {
    let (status, body) = asked_for(Some(app()), "/assets/app.js").await;
    assert_eq!(status, 200);
    assert!(body.contains("the app"), "{body}");
}

#[tokio::test]
async fn the_routes_of_a_build_with_no_app_say_there_is_none() {
    let (status, body) = asked_for(None, "/").await;
    assert_eq!(status, 404);
    assert!(body.contains("carries no web app"), "{body}");
}

#[tokio::test]
async fn a_path_the_endpoints_own_is_never_answered_with_the_app() {
    // A client fetching an endpoint that is not there is owed an absence.
    // Handing it a page instead arrives as a document it cannot parse, which
    // reads as the endpoint answering wrongly rather than as it not existing.
    for path in ["/api/status", "/api/actions/up", "/api/events"] {
        let (status, body) = asked_for(Some(app()), path).await;
        assert_eq!(status, 404, "{path}");
        assert!(
            body.contains("No endpoint answers this path"),
            "{path}: {body}"
        );
        assert!(!body.contains("<!doctype html>"), "{path}");
    }
}

#[tokio::test]
async fn a_path_that_merely_begins_like_one_is_still_the_app() {
    // The endpoints own what is below them, not every name that starts the same
    // way — the app is free to route `/apiary` for itself.
    let (status, body) = asked_for(Some(app()), "/apiary").await;
    assert_eq!(status, 200);
    assert!(body.contains("<!doctype html>"), "{body}");
}
