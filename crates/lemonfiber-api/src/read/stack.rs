//! What this stack is, what it declares, and what it is doing.
//!
//! The versions in play, the forms the stack offers, the state of every service,
//! and what those services are saying. One reading of one running stack, cut four
//! ways because the command line cuts it four ways.

use axum::extract::{RawQuery, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use lemonfiber_core::app::{logs, Ctx};
use lemonfiber_core::model::{kind, Envelope};
use lemonfiber_core::ports::docker::{LogLine, LogQuery};

use crate::reads::{Wanted, FORMS, SERVICES, STATUS, VERSION};
use crate::router::Serving;

use super::{enveloped, reading, unreadable, went_wrong, Asked};

/// Where the services' own lines are read.
///
/// The one read with no row in [`crate::reads`], because it reaches no command: it
/// opens a stream and answers with a document per line.
const LOGS: &str = "/api/logs";

/// The parameter naming a form to narrow to.
const FORM: &str = "form";
/// The parameter naming a service to read.
const SERVICE: &str = "service";
/// The parameter saying how many existing log lines to begin with.
const TAIL: &str = "tail";

/// How many existing lines a log read begins with when it is not told.
///
/// The same number the command line begins with, so the two answer a request that
/// says nothing about it with the same lines.
const BEGIN_WITH: u32 = 50;

/// What is said to a request whose line count is not a number.
const NOT_A_COUNT: &str = "How many lines to begin with must be a number.";

/// The reads about the stack itself.
pub(super) fn routes() -> Router<Serving> {
    Router::new()
        .route(VERSION, get(version))
        .route(FORMS, get(forms))
        .route(STATUS, get(status))
        .route(SERVICES, get(services))
        .route(LOGS, get(log_lines))
}

/// The versions in play: this binary, the stack it operates, and the engine's.
async fn version(State(serving): State<Serving>) -> Response {
    reading(&serving.ctx, VERSION, Wanted::default()).await
}

/// Every form the stack declares, or what naming some of them would come to.
///
/// Forms come from the stack rather than from lemonfiber, so their names are not
/// something a caller can hold in advance. Naming none lists them and naming some
/// resolves them, which is the fork `lemonfiber forms` takes on the same word.
async fn forms(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    let asked = Asked::read(query.as_deref());
    reading(
        &serving.ctx,
        FORMS,
        Wanted {
            forms: asked.every(FORM),
            ..Wanted::default()
        },
    )
    .await
}

/// What the whole stack is doing.
async fn status(State(serving): State<Serving>) -> Response {
    reading(&serving.ctx, STATUS, Wanted::default()).await
}

/// What each service is doing, narrowed to the forms that were named.
async fn services(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    let asked = Asked::read(query.as_deref());
    reading(
        &serving.ctx,
        SERVICES,
        Wanted {
            forms: asked.every(FORM),
            ..Wanted::default()
        },
    )
    .await
}

/// What the services are saying.
async fn log_lines(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    let asked = Asked::read(query.as_deref());
    let Some(tail) = counted(asked.one(TAIL)) else {
        return unreadable(NOT_A_COUNT);
    };
    read_logs(
        &serving.ctx,
        &asked.every(FORM),
        &asked.every(SERVICE),
        tail,
    )
    .await
}

/// The scrollback, one envelope per line.
///
/// A stream has no last element to close a document with, so machine-readable
/// logs are a document per line on the command line, and the same here.
///
/// The scrollback only. A read ends, and what a service says after it ended is
/// what live state is for; drawing the lines on a screen that scrolls back is a
/// thing a terminal does with them rather than a thing they are.
async fn read_logs(ctx: &Ctx, forms: &[String], services: &[String], tail: u32) -> Response {
    let query = LogQuery {
        tail,
        follow: false,
    };
    match logs(ctx, forms, services, query).await {
        Ok(mut opened) => {
            let mut said = Vec::new();
            while let Some(line) = opened.recv().await {
                said.push(line);
            }
            enveloped(StatusCode::OK, one_per_line(&said))
        }
        Err(problem) => went_wrong(&problem),
    }
}

/// Every line as its own envelope, or nothing where one could not be rendered.
fn one_per_line(said: &[LogLine]) -> Option<String> {
    said.iter()
        .map(|line| {
            Envelope::new(kind::LOG, line)
                .to_json()
                .map(|json| json + "\n")
        })
        .collect()
}

/// How many existing lines to begin with, or nothing where what was asked for is
/// not a number.
fn counted(said: Option<&str>) -> Option<u32> {
    said.map_or(Some(BEGIN_WITH), |said| said.parse().ok())
}
