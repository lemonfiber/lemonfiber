//! What the checks found.
//!
//! One diagnosis, whole or narrowed. The disk has an endpoint of its own because
//! the dashboard asks about it on its own, and it is the same group of checks the
//! narrowing parameter reaches — two names for one answer rather than two gathers
//! that can disagree.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::{Wanted, CHECKS, STORAGE};
use crate::router::Serving;

use super::{reading, Asked};

/// The parameter naming what the checks are narrowed to.
const ONLY: &str = "only";

/// The reads that run the diagnostic checks.
pub(super) fn routes() -> Router<Serving> {
    Router::new()
        .route(CHECKS, get(checks))
        .route(STORAGE, get(storage))
}

/// What the diagnostic checks found, or one group of them.
async fn checks(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    let asked = Asked::read(query.as_deref());
    reading(
        &serving.ctx,
        CHECKS,
        Wanted {
            only: asked.one(ONLY).map(str::to_owned),
            ..Wanted::default()
        },
    )
    .await
}

/// What the checks about the disk found.
async fn storage(State(serving): State<Serving>) -> Response {
    reading(&serving.ctx, STORAGE, Wanted::default()).await
}
