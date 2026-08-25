//! What the checks found.
//!
//! Two reads over one diagnosis, whole or narrowed. The disk has an endpoint of its
//! own because
//! the dashboard asks about it on its own, and it is the same group of checks the
//! narrowing parameter reaches — two names for one answer rather than two gathers
//! that can disagree.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::{CHECKS, STORAGE};
use crate::router::Serving;

use super::reading;

/// The reads that run the diagnostic checks.
pub(super) fn routes() -> Router<Serving> {
    Router::new()
        .route(CHECKS, get(checks))
        .route(STORAGE, get(storage))
}

/// What the diagnostic checks found, or one group of them.
async fn checks(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, CHECKS, query.as_deref()).await
}

/// What the checks about the disk found.
async fn storage(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, STORAGE, query.as_deref()).await
}
