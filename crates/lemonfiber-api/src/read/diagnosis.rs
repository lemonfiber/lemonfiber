//! What the checks found.
//!
//! One diagnosis, whole or narrowed. The disk has an endpoint of its own because
//! the dashboard asks about it on its own, and it is the same group of checks the
//! narrowing parameter reaches — two names for one answer rather than two gathers
//! that can disagree.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use lemonfiber_core::app::Command;
use lemonfiber_core::doctor::{Category, Narrowing};

use crate::router::Serving;

use super::{carried_out, unreadable, Asked};

/// The parameter naming what the checks are narrowed to.
const ONLY: &str = "only";

/// What is said to a request naming a group of checks that is not one.
const NO_SUCH_GROUP: &str = "There is no group of checks and no check by that name.";

/// The reads that run the diagnostic checks.
pub(super) fn routes() -> Router<Serving> {
    Router::new()
        .route("/api/checks", get(checks))
        .route("/api/storage", get(storage))
}

/// What the diagnostic checks found, or one group of them.
async fn checks(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    let asked = Asked::read(query.as_deref());
    let Some(command) = diagnosis(asked.one(ONLY)) else {
        return unreadable(NO_SUCH_GROUP);
    };
    carried_out(&serving.ctx, command).await
}

/// What the checks about the disk found.
async fn storage(State(serving): State<Serving>) -> Response {
    carried_out(
        &serving.ctx,
        diagnosing(Narrowing::Category(Category::Storage)),
    )
    .await
}

/// A diagnosis, narrowed or whole.
///
/// A read looks and does not touch, so it neither accepts a warning nor opts into
/// the checks that disturb a running system; both of those change something.
const fn diagnosing(narrowing: Narrowing) -> Command {
    Command::Doctor {
        narrowing,
        disruptive: false,
        accept: None,
    }
}

/// The diagnosis a request asked for, or nothing where it named a group of checks
/// that is not one lemonfiber knows.
fn diagnosis(only: Option<&str>) -> Option<Command> {
    match only {
        None => Some(diagnosing(Narrowing::Suite)),
        Some(name) => Narrowing::parse(name).map(diagnosing),
    }
}
