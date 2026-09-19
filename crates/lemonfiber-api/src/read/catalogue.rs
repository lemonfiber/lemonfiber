//! The one read about what each service is for.
//!
//! Served rather than assembled, for the reason the outbound enumeration is: what a
//! service does for the operator is written in the stack description this machine
//! actually runs, and a page that carried its own copy of the descriptions would be
//! describing whichever stack its author had in mind rather than the one in front of
//! whoever is reading it.
//!
//! It takes nothing. What is in this stack is a property of the stack, so there is no
//! narrowing a caller could ask for — and an operator who cannot tell which of nineteen
//! names matters is asking about all of them.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::admission::Caller;
use crate::reads::CATALOGUE;
use crate::router::Serving;

use super::reading;

/// The read about what each service is for.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(CATALOGUE, get(catalogue))
}

/// Every service this stack declares, and every one it has dropped.
async fn catalogue(
    State(serving): State<Serving>,
    caller: Caller,
    RawQuery(query): RawQuery,
) -> Response {
    reading(&serving.ctx, &caller, CATALOGUE, query.as_deref()).await
}
