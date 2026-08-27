//! The one read about what this machine keeps of lemonfiber's.
//!
//! A browser has no filesystem in front of it, so this is the read it cannot answer
//! for itself at all: where lemonfiber's own files are on the host, what each of
//! them is, and which of them hold a credential. Served rather than assembled, so
//! the answer a page draws is the answer a shell prints.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::STORED;
use crate::router::Serving;

use super::reading;

/// The read about what is kept on this machine.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(STORED, get(stored))
}

/// Everything lemonfiber keeps on this machine, where each thing is and why.
async fn stored(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, STORED, query.as_deref()).await
}
