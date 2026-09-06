//! The one read about what this machine keeps running when nobody is watching.
//!
//! A page cannot ask a service manager whether it is running something, and that
//! is the whole of what this answers: not that a definition was written, but that
//! the machine confirms it is keeping the command going. So it is served rather
//! than assembled, and a browser sees exactly what a shell prints.
//!
//! It reaches the command with nothing asked for, which is what makes it a read.
//! Installing one and taking it back are two requests at the door changes are
//! asked for, each naming which command it means.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::HOSTING;
use crate::router::Serving;

use super::reading;

/// The read about what this machine keeps running.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(HOSTING, get(hosting))
}

/// What stands between each long-running command and this machine.
async fn hosting(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, HOSTING, query.as_deref()).await
}
