//! The one read about the credentials this stack holds.
//!
//! The reading half of the word, and deliberately only that half. What comes back
//! names every credential, what authenticates with it, where its value lives and
//! where it stands — and carries no value, because the shape it is built from has
//! nowhere to put one.
//!
//! Replacing a credential and printing one are not offered here at all, and there is
//! no action beside this read that offers them either. A credential sent through this
//! door would pass a browser's cache, whatever proxy is in between, and the log each
//! of them keeps; and a request that could ask for one is a request somebody can be
//! tricked into making. The terminal is where a value is printed, in front of the
//! person who typed the confirmation.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::CREDENTIALS;
use crate::router::Serving;

use super::reading;

/// The read about the credentials this stack holds.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(CREDENTIALS, get(credentials))
}

/// Every credential, with none of their values.
async fn credentials(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    reading(&serving.ctx, CREDENTIALS, query.as_deref()).await
}
