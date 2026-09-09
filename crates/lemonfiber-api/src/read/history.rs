//! The one read about what lemonfiber has already changed.
//!
//! The record only. What comes back is every journalled change newest first, what each
//! did in the operator's own terms, and how far each could be put back — whole, in part,
//! or not at all, with the reason where it is not.
//!
//! Putting a change back is not offered here. It is an act on a running stack, so it
//! belongs behind a named action rather than a door a browser opens by asking for it.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::HISTORY;
use crate::router::Serving;

use super::reading;

/// The read about what lemonfiber has already changed.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(HISTORY, get(history))
}

/// Every change on record, and how far each could be put back.
async fn history(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, HISTORY, query.as_deref()).await
}
