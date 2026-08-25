//! What the household asked for, where one item got to, and what has stopped.
//!
//! Three reads in the order a reader takes them. The household's requests are the
//! list, following one of them is the next question, and the items whose downloads
//! have stopped are the same question arrived at from the other end — which is why
//! each entry a stuck read reports is named the way this trace is asked for.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use lemonfiber_core::app::Command;

use crate::router::Serving;

use super::{carried_out, unreadable, Asked};

/// The parameter naming the household member to narrow to.
const MEMBER: &str = "member";
/// The parameter naming what to follow.
const TERM: &str = "term";
/// The parameter naming the season to narrow the per-part coverage to.
const SEASON: &str = "season";

/// What is said to a request that named nothing to follow.
const NO_TERM: &str = "What to follow must be named.";

/// What is said to a request whose season is not a number.
const NOT_A_SEASON: &str = "Which season to narrow to must be a number.";

/// The reads about what was asked for and where it got to.
pub(super) fn routes() -> Router<Serving> {
    Router::new()
        .route("/api/requests", get(requests))
        .route("/api/trace", get(trace))
        .route("/api/stuck", get(stuck))
}

/// What the household has asked for, and where each request stands.
async fn requests(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    let asked = Asked::read(query.as_deref());
    carried_out(
        &serving.ctx,
        Command::Household {
            member: asked.one(MEMBER).map(str::to_owned),
        },
    )
    .await
}

/// Where one item is, followed by the words a person would name it with.
///
/// The term is one parameter rather than several. The command line takes it as
/// words so it can be typed without quoting and joins them back into the title as
/// said; a query string carries the title already whole.
async fn trace(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    let asked = Asked::read(query.as_deref());
    let Some(term) = asked.one(TERM).filter(|term| !term.is_empty()) else {
        return unreadable(NO_TERM);
    };
    let Ok(season) = asked.one(SEASON).map(str::parse::<u32>).transpose() else {
        return unreadable(NOT_A_SEASON);
    };
    carried_out(
        &serving.ctx,
        Command::Trace {
            term: term.to_owned(),
            season,
        },
    )
    .await
}

/// The items whose downloads have stopped, each named so it can be followed.
async fn stuck(State(serving): State<Serving>) -> Response {
    carried_out(&serving.ctx, Command::Stuck).await
}
