//! The one address to hand somebody who lives here.
//!
//! One read, and its own module because what it is about is its own question. Every
//! other read here answers something an operator asks about their stack; this
//! answers what they send to somebody who does not operate it — which is why the
//! answer names one service and says why nothing else it lists is that service.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::FRONT_DOOR;
use crate::router::Serving;

use super::reading;

/// The read about where the household begins.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(FRONT_DOOR, get(front_door))
}

/// Which service the household is sent to, where it stands, and what is not it.
async fn front_door(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, FRONT_DOOR, query.as_deref()).await
}
