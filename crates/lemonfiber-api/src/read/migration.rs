//! The one read about what is already on this machine.
//!
//! A survey and nothing else. What comes back names the projects already standing here,
//! the ports they hold that lemonfiber would want, and what it could not take over.
//!
//! Choosing what to do about any of it is not offered here. Adopting a stack, standing
//! one beside it, or replacing it are acts on somebody's existing setup, so they belong
//! behind named actions rather than a door a browser opens by asking for it.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::MIGRATION;
use crate::router::Serving;

use super::reading;

/// The read about what is already here.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(MIGRATION, get(migration))
}

/// What is standing on this machine, and what of it lemonfiber could take over.
async fn migration(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, MIGRATION, query.as_deref()).await
}
