//! The one read about what the operator is told about.
//!
//! The reading half only. What comes back names the preset in force, what that preset
//! means in the operator's own terms, and any event kind set apart from it.
//!
//! Choosing a preset is not offered here. It is an act on what reaches somebody —
//! quieter, and a fault goes unmentioned until they next look — so it belongs behind a
//! named action rather than a door a browser opens by asking for it.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::ALERTS;
use crate::router::Serving;

use super::reading;

/// The read about what the operator is told about.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(ALERTS, get(alerts))
}

/// The preset in force, what it means, and what is set apart from it.
async fn alerts(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, ALERTS, query.as_deref()).await
}
