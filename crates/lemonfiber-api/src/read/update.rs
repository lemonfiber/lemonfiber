//! The one read about where this copy of lemonfiber stands.
//!
//! A read and never a replacement. What it answers with is the exact command for
//! whichever tool owns the copy that is running — something a page can put in front of
//! an operator to copy, and never something this surface carries out, because a browser
//! request that replaced the program serving it is the shape of a thing nobody should
//! be able to ask for by accident.
//!
//! Served rather than assembled, so the answer a page draws is the answer a shell
//! prints. A browser cannot see the host at all, and which of several copies on a
//! search path is running is exactly the kind of thing only the process can say.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::UPDATE;
use crate::router::Serving;

use super::reading;

/// The read about where this copy stands.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(UPDATE, get(update))
}

/// Where this copy of lemonfiber stands, and what moving it would come to.
async fn update(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, UPDATE, query.as_deref()).await
}
