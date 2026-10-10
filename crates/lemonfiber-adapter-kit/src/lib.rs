//! What a plugin's adapter runs to speak lemonfiber's capability contracts.
//!
//! An adapter implements the ports of each capability it fills and hands them to a
//! [`Kit`]. The kit answers every request the core makes: each operation at its
//! contract's path, `about` and `ready`, and every one of them only under the plugin's
//! key, which the core wrote to the adapter's configuration directory before it started.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::extract::{Path as Segments, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use lemonfiber_contract::adapter::{about_path, ready_path, About, KEY_FILE, UPSTREAM_FILE};
use lemonfiber_contract::wire::{JSON, PROBLEM};
use lemonfiber_contract::{Refusal, Served};
use tokio::net::TcpListener;

/// Whether the upstream answers, as an adapter finds out.
pub type Readiness = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;

/// An adapter's contracts, served under its plugin's key.
#[derive(Clone)]
pub struct Kit {
    key: String,
    about: About,
    ready: Readiness,
    served: Vec<Served>,
}

impl Kit {
    /// An adapter that answers under `key`, says it is `about`, serves nothing yet and
    /// says its upstream answers.
    #[must_use]
    pub fn new(key: impl Into<String>, about: About) -> Self {
        Self {
            key: key.into(),
            about,
            ready: Arc::new(|| Box::pin(async { true })),
            served: Vec::new(),
        }
    }

    /// The same adapter, saying whether its upstream answers by asking `ready`.
    #[must_use]
    pub fn ready_when(mut self, ready: Readiness) -> Self {
        self.ready = ready;
        self
    }

    /// The same adapter, serving one more capability.
    #[must_use]
    pub fn serving(mut self, served: Served) -> Self {
        self.served.push(served);
        self
    }

    /// Every route the core asks.
    pub fn router(self) -> Router {
        Router::new()
            .route(&about_path(), get(about))
            .route(&ready_path(), get(ready))
            .route(
                "/lemonfiber/{capability}/{major}/{operation}",
                post(operation),
            )
            .with_state(Arc::new(self))
    }
}

/// The plugin's key, as the core wrote it to the adapter's configuration directory.
///
/// # Errors
///
/// The error reading it, where the core wrote none.
pub fn key_in(configuration: &Path) -> std::io::Result<String> {
    std::fs::read_to_string(configuration.join(KEY_FILE)).map(|key| key.trim().to_owned())
}

/// The upstream's credential, as the core last wrote it to the adapter's configuration
/// directory: each of its parts by name.
///
/// Read again on each call, because the core rewrites it when it rotates or moves that
/// credential and the adapter never does.
///
/// # Errors
///
/// The error reading it, where the core wrote none, or [`std::io::ErrorKind::InvalidData`]
/// where what is there is not a JSON object of strings.
pub fn upstream_in(configuration: &Path) -> std::io::Result<BTreeMap<String, String>> {
    let written = std::fs::read(configuration.join(UPSTREAM_FILE))?;
    serde_json::from_slice(&written)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}

/// Answer the core at `address` until the process ends.
///
/// # Errors
///
/// The error binding `address` or serving on it.
pub async fn serve(kit: Kit, address: &str) -> std::io::Result<()> {
    serving(
        kit,
        TcpListener::bind(address).await?,
        std::future::pending(),
    )
    .await
}

/// Answer the core on `listener` until `stopped` completes.
///
/// # Errors
///
/// The error serving on it.
pub async fn serving(
    kit: Kit,
    listener: TcpListener,
    stopped: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(listener, kit.router())
        .with_graceful_shutdown(stopped)
        .await
}

/// What the adapter says it is.
async fn about(State(kit): State<Arc<Kit>>, headers: HeaderMap) -> Response {
    if !keyed(&kit, &headers) {
        return unkeyed();
    }
    json(&kit.about)
}

/// Whether its upstream answers.
async fn ready(State(kit): State<Arc<Kit>>, headers: HeaderMap) -> Response {
    if !keyed(&kit, &headers) {
        return unkeyed();
    }
    json(&(kit.ready)().await)
}

/// One operation of one capability, served from what fills it.
async fn operation(
    State(kit): State<Arc<Kit>>,
    Segments((capability, major, operation)): Segments<(String, String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !keyed(&kit, &headers) {
        return unkeyed();
    }
    let Some(served) = kit
        .served
        .iter()
        .find(|served| served.capability == capability && format!("v{}", served.major) == major)
    else {
        return refused(&Refusal::unknown_operation(&operation));
    };
    match served.dispatch(&operation, body.to_vec()).await {
        Ok(answer) => answered(StatusCode::OK, Some(JSON), answer),
        Err(refusal) => refused(&refusal),
    }
}

/// Whether a request carries the plugin's key, compared in time that does not depend on
/// where the two first differ.
fn keyed(kit: &Kit, headers: &HeaderMap) -> bool {
    let offered = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default()
        .as_bytes();
    let mine = kit.key.as_bytes();
    !mine.is_empty()
        && mine.len() == offered.len()
        && mine
            .iter()
            .zip(offered)
            .fold(0, |differs, (one, other)| differs | (one ^ other))
            == 0
}

/// A request without the plugin's key.
fn unkeyed() -> Response {
    answered(StatusCode::UNAUTHORIZED, None, Vec::new())
}

/// A refusal, as the problem document the contract declares.
fn refused(refusal: &Refusal) -> Response {
    let status = StatusCode::from_u16(refusal.kind.status()).unwrap_or(StatusCode::BAD_GATEWAY);
    answered(
        status,
        Some(PROBLEM),
        serde_json::to_vec(refusal).unwrap_or_default(),
    )
}

/// A JSON answer.
fn json(answer: &impl serde::Serialize) -> Response {
    answered(
        StatusCode::OK,
        Some(JSON),
        serde_json::to_vec(answer).unwrap_or_default(),
    )
}

/// A response with `status`, typed as `sort`, carrying `body`.
fn answered(status: StatusCode, sort: Option<&'static str>, body: Vec<u8>) -> Response {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    if let Some(sort) = sort {
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(sort));
    }
    response
}

#[cfg(test)]
mod tests;
