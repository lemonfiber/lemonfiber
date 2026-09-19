//! The one read about where the services come from.
//!
//! Served rather than assembled, for the reason the outbound enumeration is: the
//! answer is read out of the stack description this machine actually runs, and a page
//! that carried its own copy of the licences would be a second account of what is
//! bundled — which is exactly the thing an operator is here to check rather than to
//! be told twice.
//!
//! It takes nothing. What is in this stack is a property of the stack, so there is no
//! narrowing a caller could ask for and no half of the answer anybody could be shown.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::admission::Caller;
use crate::reads::PROVENANCE;
use crate::router::Serving;

use super::reading;

/// The read about where each service comes from.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(PROVENANCE, get(provenance))
}

/// Every service this stack declares, with its licence, its project and its pin.
async fn provenance(
    State(serving): State<Serving>,
    caller: Caller,
    RawQuery(query): RawQuery,
) -> Response {
    reading(&serving.ctx, &caller, PROVENANCE, query.as_deref()).await
}
