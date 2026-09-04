//! The one read about which app to watch on.
//!
//! Takes no arguments: the table is the same on every machine, so this needs
//! neither a stack running nor a daemon reachable. The one caution above it is read
//! from the quality choice and the platform, best-effort, so a machine with nothing
//! set up is still answered in full.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::CLIENTS;
use crate::router::Serving;

use super::reading;

/// The read about which app to use on which device.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(CLIENTS, get(clients))
}

/// Which app to use on each kind of device, and where the answer is to use
/// something else instead.
async fn clients(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, CLIENTS, query.as_deref()).await
}
