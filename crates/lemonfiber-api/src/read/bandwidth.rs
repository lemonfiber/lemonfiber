//! The one read about how the line is shared.
//!
//! A page can be told a number; it cannot sign in to a download client, read what
//! that client is limited to, and read what it is moving beside it — and the whole
//! answer here is built out of those three. So it is served rather than assembled,
//! and a browser sees exactly the account a shell prints.
//!
//! It reaches the command with nothing asked for, which is what makes it a read.
//! Declaring a limit is a different request at the door changes are asked for,
//! carrying the same words.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::BANDWIDTH;
use crate::router::Serving;

use super::reading;

/// The read about how the line is shared.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(BANDWIDTH, get(bandwidth))
}

/// What the line carries, what the stack may take of it, and what the clients say.
async fn bandwidth(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, BANDWIDTH, query.as_deref()).await
}
