//! The one read about everything that leaves this machine.
//!
//! The read a browser has the most reason to want and the least ability to answer
//! for itself: what a page can see of its own traffic is what the page itself asks
//! for, and nothing at all of what the process behind it does. So it is served
//! rather than assembled — one list, from the same command a shell types, over the
//! settings this machine actually holds.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::OUTBOUND;
use crate::router::Serving;

use super::reading;

/// The read about what leaves this machine.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(OUTBOUND, get(outbound))
}

/// Every request lemonfiber makes on its own account, and every one the stack's own
/// services make, with the setting that switches each of lemonfiber's off.
async fn outbound(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, OUTBOUND, query.as_deref()).await
}
