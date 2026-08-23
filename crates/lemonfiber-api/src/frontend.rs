//! Serving the app's files, and saying what each one is.
//!
//! The other half of [`lemonfiber_core::frontend`]. Which file a path names is a
//! question about the app and is answered there; what a browser is told that file
//! *is*, and what it may do with it, is a question about HTTP and is answered
//! here. The core has no HTTP server and must not gain one.
//!
//! Nothing served here is cached. The app declares the version of the contract it
//! speaks, so a browser holding yesterday's copy is a browser drawing today's
//! fields under yesterday's meanings — and these bytes come out of memory on
//! loopback, where a cache saves nothing worth that.
//!
//! The token an operator was given reaches this page, so the page says what it may
//! load and refuses to be framed. Neither costs anything to state and both stop
//! being available to state once somebody is already relying on the absence.

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderValue, Response, StatusCode};
use axum::routing::any;
use axum::Router;
use lemonfiber_core::frontend::{Asset, Source};

use crate::serve::{carrying, SENTENCE};

/// What this page may load, and what may load it.
///
/// Styles are allowed inline because a build inlines the first paint's rules to
/// avoid a flash of unstyled text; scripts are not, because nothing needs it.
const POLICY: &str = "default-src 'self'; img-src 'self' data:; \
                      style-src 'self' 'unsafe-inline'; font-src 'self'; \
                      connect-src 'self'; base-uri 'none'; frame-ancestors 'none'";

/// What a browser is told a file is, from what the file is called.
///
/// An extension nobody here claims is handed over as bytes rather than guessed
/// at. A wrong type is acted on; an unknown one is downloaded, which is the safe
/// direction for a file this surface did not expect to be serving.
#[must_use]
pub fn content_type(path: &std::path::Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// One file of the app, with what it is and what it may do.
#[must_use]
pub fn served(asset: &Asset) -> Response<Body> {
    let body = Body::from(asset.bytes.clone().into_owned());
    let mut response = carrying(StatusCode::OK, content_type(&asset.path), body);
    allowed(&mut response);
    response
}

/// There is an app, and it holds no such file.
#[must_use]
pub fn missing() -> Response<Body> {
    plainly(StatusCode::NOT_FOUND, "This app holds no such file.")
}

/// There is no app in this build to serve anything from.
///
/// Said rather than answered with an empty document, because the two are
/// different faults: one is a path nobody wrote, and this is a binary built
/// without the surface it is being asked for.
#[must_use]
pub fn absent() -> Response<Body> {
    plainly(
        StatusCode::NOT_FOUND,
        "This build of lemonfiber carries no web app. The endpoints below /api answer as usual.",
    )
}

/// Where the endpoints answer, and the app therefore does not.
const ENDPOINTS: &str = "/api/";

/// A path below the endpoints that no endpoint answered.
///
/// Never the app. A client fetching an endpoint that is not there is owed an
/// absence, and handing it a page instead would arrive as a document it cannot
/// parse — a fault that reads as the endpoint answering wrongly rather than as
/// the endpoint not existing.
#[must_use]
pub fn unanswered() -> Response<Body> {
    plainly(StatusCode::NOT_FOUND, "No endpoint answers this path.")
}

/// What a path is answered with, whichever of the four it is.
#[must_use]
pub fn page(app: Option<Source>, asked: &str) -> Response<Body> {
    if asked.starts_with(ENDPOINTS) {
        return unanswered();
    }
    let Some(source) = app.filter(|source| source.holds_an_app()) else {
        return absent();
    };
    source
        .asset(asked)
        .map_or_else(missing, |asset| served(&asset))
}

/// The routes that serve the app.
///
/// One fallback rather than a route per file. The app's own router reads the path
/// once the page is loaded, so every path this surface does not answer itself is
/// the app — which means the endpoints must be merged over this rather than
/// under it.
pub fn routes(app: Option<Source>) -> Router {
    Router::new()
        .fallback(any(
            |State(app): State<Option<Source>>, request: axum::extract::Request| async move {
                page(app, request.uri().path())
            },
        ))
        .with_state(app)
}

/// A response carrying words this surface wrote.
///
/// Through the same place every other answer is built, so it wears the same
/// headers, and labelled as the prose it is.
fn plainly(status: StatusCode, said: &str) -> Response<Body> {
    let mut response = carrying(status, SENTENCE, Body::from(said.to_owned()));
    allowed(&mut response);
    response
}

/// What the page this surface serves may load, and what may load it.
///
/// Only the app's own answers carry it. The endpoints beside them are fetched by
/// a page rather than drawn as one, and a policy on a document nothing renders
/// says nothing about what anything may do.
fn allowed(response: &mut Response<Body>) {
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(POLICY),
    );
}
