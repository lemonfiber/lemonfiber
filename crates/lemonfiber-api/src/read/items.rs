//! What the household asked for, where one item got to, and what has stopped.
//!
//! Three reads in the order a reader takes them. The household's requests are the
//! list, following one of them is the next question, and the items whose downloads
//! have stopped are the same question arrived at from the other end — which is why
//! each entry a stuck read reports is named the way this trace is asked for.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::{REQUESTS, STUCK, TRACE};
use crate::router::Serving;

use super::reading;

/// The reads about what was asked for and where it got to.
pub(super) fn routes() -> Router<Serving> {
    Router::new()
        .route(REQUESTS, get(requests))
        .route(TRACE, get(trace))
        .route(STUCK, get(stuck))
}

/// What the household has asked for, and where each request stands.
async fn requests(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, REQUESTS, query.as_deref()).await
}

/// Where one item is, followed by the words a person would name it with.
async fn trace(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, TRACE, query.as_deref()).await
}

/// The items whose downloads have stopped, each named so it can be followed.
async fn stuck(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, STUCK, query.as_deref()).await
}
