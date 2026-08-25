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

use crate::reads::{Wanted, REQUESTS, STUCK, TRACE};
use crate::router::Serving;

use super::{reading, Asked};

/// The parameter naming the household member to narrow to.
const MEMBER: &str = "member";
/// The parameter naming what to follow.
const TERM: &str = "term";
/// The parameter naming the season to narrow the per-part coverage to.
const SEASON: &str = "season";

/// The reads about what was asked for and where it got to.
pub(super) fn routes() -> Router<Serving> {
    Router::new()
        .route(REQUESTS, get(requests))
        .route(TRACE, get(trace))
        .route(STUCK, get(stuck))
}

/// What the household has asked for, and where each request stands.
async fn requests(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    let asked = Asked::read(query.as_deref());
    reading(
        &serving.ctx,
        REQUESTS,
        Wanted {
            member: asked.one(MEMBER).map(str::to_owned),
            ..Wanted::default()
        },
    )
    .await
}

/// Where one item is, followed by the words a person would name it with.
async fn trace(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    let asked = Asked::read(query.as_deref());
    reading(
        &serving.ctx,
        TRACE,
        Wanted {
            term: asked.one(TERM).map(str::to_owned),
            season: asked.one(SEASON).map(str::to_owned),
            ..Wanted::default()
        },
    )
    .await
}

/// The items whose downloads have stopped, each named so it can be followed.
async fn stuck(State(serving): State<Serving>) -> Response {
    reading(&serving.ctx, STUCK, Wanted::default()).await
}
