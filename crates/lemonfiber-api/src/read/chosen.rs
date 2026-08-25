//! The choices in force: what the settings say, and what quality was asked for.
//!
//! The two reads whose writes this surface already offered. A browser could change
//! a setting and choose a preset before it could read either back, which meant it
//! could write a value it had no way to confirm.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::{Wanted, CONFIG, QUALITY};
use crate::router::Serving;

use super::{reading, Asked};

/// The parameter naming one setting to read, instead of all of them.
const KEY: &str = "key";

/// The reads about what has been chosen.
pub(super) fn routes() -> Router<Serving> {
    Router::new()
        .route(CONFIG, get(config))
        .route(QUALITY, get(quality))
}

/// Every setting, or one of them by name, with credentials withheld.
///
/// The withholding happens in the core, where the settings are read: a value nobody
/// has written down a reason for showing is replaced before any report carries it, so
/// this endpoint and `lemonfiber config show` withhold the same values.
///
/// Naming none shows them all and naming one reads that one, which is the fork
/// `config show` and `config get` take on the command line.
async fn config(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    let asked = Asked::read(query.as_deref());
    reading(
        &serving.ctx,
        CONFIG,
        Wanted {
            key: asked.one(KEY).map(str::to_owned),
            ..Wanted::default()
        },
    )
    .await
}

/// The quality choice in force, what each preset means, and what it costs.
async fn quality(State(serving): State<Serving>) -> Response {
    reading(&serving.ctx, QUALITY, Wanted::default()).await
}
