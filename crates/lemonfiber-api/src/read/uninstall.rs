//! The one read about taking lemonfiber off this machine.
//!
//! A read and not a removal. What it answers with is the listing — every container,
//! image and path a removal would take, with what each occupies — and the action of
//! the same name is where an answer to that listing goes. A browser is shown what a
//! shell prints, so the thing agreed to on one surface is the thing read on the
//! other.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::UNINSTALL;
use crate::router::Serving;

use super::reading;

/// The read about what a removal would take.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(UNINSTALL, get(uninstall))
}

/// What one of the four removals would take, and nothing removed.
async fn uninstall(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, UNINSTALL, query.as_deref()).await
}
