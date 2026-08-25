//! The choices in force: what the settings say, and what quality was asked for.
//!
//! The two reads whose writes this surface already offered. A browser could change
//! a setting and choose a preset before it could read either back, which meant it
//! could write a value it had no way to confirm.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use lemonfiber_core::app::{Command, QualityAction};

use crate::router::Serving;

use super::{carried_out, Asked};

/// The parameter naming one setting to read, instead of all of them.
const KEY: &str = "key";

/// The reads about what has been chosen.
pub(super) fn routes() -> Router<Serving> {
    Router::new()
        .route("/api/config", get(config))
        .route("/api/quality", get(quality))
}

/// Every setting, or one of them by name, with credentials withheld.
///
/// The withholding happens in the core, where the settings are read: a value whose
/// name reads as a credential is replaced before any report carries it, so this
/// endpoint and `lemonfiber config show` withhold the same values.
///
/// Naming none shows them all and naming one reads that one, which is the fork
/// `config show` and `config get` take on the command line.
async fn config(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    let asked = Asked::read(query.as_deref());
    let command = match asked.one(KEY) {
        Some(key) => Command::ConfigGet {
            key: key.to_owned(),
        },
        None => Command::ConfigShow,
    };
    carried_out(&serving.ctx, command).await
}

/// The quality choice in force, what each preset means, and what it costs.
async fn quality(State(serving): State<Serving>) -> Response {
    carried_out(&serving.ctx, Command::Quality(QualityAction::Show)).await
}
