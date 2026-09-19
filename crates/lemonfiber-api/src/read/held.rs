//! The one read that answers what somebody can watch, rather than what they asked for.
//!
//! A member opens an app for two questions, and until this there was only ever an
//! answer to one of them. The household read says what has been asked for and where
//! each request stands; this says what is already there.
//!
//! **Whose shelf is the request, and for a member it is not theirs to choose.** A
//! member's session names them, and what leaves the entitlement decision carries that
//! name whatever the query string said — so there is no request, hand-written or
//! otherwise, that reaches this with somebody else's shelf on it.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::admission::Caller;
use crate::reads::HELD;
use crate::router::Serving;

use super::reading;

/// The read about what one member can watch.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(HELD, get(held))
}

/// What is on one member's shelf.
async fn held(
    State(serving): State<Serving>,
    caller: Caller,
    RawQuery(query): RawQuery,
) -> Response {
    reading(&serving.ctx, &caller, HELD, query.as_deref()).await
}
