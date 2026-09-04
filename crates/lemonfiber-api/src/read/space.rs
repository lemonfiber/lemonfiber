//! The one read about where the disk went.
//!
//! A page can be told how much room is left; it cannot walk a tree, count the
//! names pointing at a file, or ask a download client what it is still seeding —
//! and the whole answer here is built out of those three. So it is served rather
//! than assembled, and a browser sees exactly the account a shell prints.
//!
//! It reaches the command with nothing confirmed, which is what makes it a read.
//! Agreeing to what the account offered is a different request at the door changes
//! are asked for, carrying the same word.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::SPACE;
use crate::router::Serving;

use super::reading;

/// The read about where the disk went.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(SPACE, get(space))
}

/// Where the disk stands, where the room went, and what could be got back.
async fn space(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, SPACE, query.as_deref()).await
}
