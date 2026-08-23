//! The six reads: one endpoint per question a command already answers.
//!
//! Nothing here serialises anything. An endpoint turns its path and its query
//! into the command a person would type, hands that to the dispatcher the
//! command line hands it to, and answers with the envelope that command renders.
//! The two surfaces therefore cannot say different things about the same stack,
//! because there is only one rendering and both of them read it.
//!
//! Query parameters are the commands' own flags. A read takes only the flags
//! that read: narrowing a diagnosis is a parameter here, while accepting a
//! warning or running the checks that disturb a running system changes something
//! and belongs where changes are asked for.
//!
//! Four commands answer six endpoints. `status` and `services` are one reading of
//! what is running, whole and narrowed to named forms; `storage` is the group of
//! checks about the disk, which `checks` will also narrow to. Two endpoints over
//! one command is two names for one answer, which is the opposite of two gathers
//! that can disagree.

use axum::extract::{RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use lemonfiber_core::app::{dispatch, logs, Command, Ctx};
use lemonfiber_core::doctor::Category;
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::{kind, Envelope};
use lemonfiber_core::ports::docker::{LogLine, LogQuery};

use crate::router::Serving;
use crate::serve::answered;

/// The parameter naming a form to narrow to.
const FORM: &str = "form";
/// The parameter naming a service to read.
const SERVICE: &str = "service";
/// The parameter naming the group of checks to run.
const ONLY: &str = "only";
/// The parameter naming the household member to narrow to.
const MEMBER: &str = "member";
/// The parameter saying how many existing log lines to begin with.
const TAIL: &str = "tail";

/// How many existing lines a log read begins with when it is not told.
///
/// The same number the command line begins with, so the two answer a request that
/// says nothing about it with the same lines.
const BEGIN_WITH: u32 = 50;

/// The status a command that could not be carried out is answered with.
///
/// The body is still the envelope, because a caller that asked for something it
/// could parse asked about the failures most of all.
const FAILED: StatusCode = StatusCode::INTERNAL_SERVER_ERROR;

/// What is said to a request naming a group of checks that is not one.
const NO_SUCH_GROUP: &str = "There is no group of checks by that name.";

/// What is said to a request whose line count is not a number.
const NOT_A_COUNT: &str = "How many lines to begin with must be a number.";

/// What is said where a payload could not be rendered.
const UNRENDERABLE: &str = "This answer could not be rendered.";

/// The reads this surface answers.
pub fn routes() -> Router<Serving> {
    Router::new()
        .route("/api/status", get(status))
        .route("/api/services", get(services))
        .route("/api/checks", get(checks))
        .route("/api/storage", get(storage))
        .route("/api/logs", get(log_lines))
        .route("/api/requests", get(requests))
}

/// What the whole stack is doing.
async fn status(State(serving): State<Serving>) -> Response {
    carried_out(&serving.ctx, Command::Ps { forms: Vec::new() }).await
}

/// What each service is doing, narrowed to the forms that were named.
async fn services(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    let asked = Asked::read(query.as_deref());
    carried_out(
        &serving.ctx,
        Command::Ps {
            forms: asked.every(FORM),
        },
    )
    .await
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
    carried_out(&serving.ctx, diagnosing(Some(Category::Storage))).await
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

/// A diagnosis, narrowed or whole.
///
/// A read looks and does not touch, so it neither accepts a warning nor opts into
/// the checks that disturb a running system; both of those change something.
const fn diagnosing(only: Option<Category>) -> Command {
    Command::Doctor {
        only,
        disruptive: false,
        accept: None,
    }
}

/// Carry out a command and answer with the envelope it renders.
///
/// The three calls a machine-readable command line makes, in the order it makes
/// them, so the bytes a caller reads here are the bytes it would have piped.
pub async fn carried_out(ctx: &Ctx, command: Command) -> Response {
    match dispatch(command, ctx).await {
        Ok(outcome) => enveloped(StatusCode::OK, outcome.envelope().to_json()),
        Err(problem) => went_wrong(&problem),
    }
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

/// The failure a command reported, in the envelope machine-readable output gives
/// it.
fn went_wrong(problem: &Problem) -> Response {
    enveloped(FAILED, Envelope::new(kind::ERROR, problem).to_json())
}

/// A rendered envelope as a response, at the status the answer warrants.
///
/// Built by [`answered`], which is where a body this surface produces is given
/// its headers; only the status differs, since a command that could not be
/// carried out is not a successful read.
///
/// Nothing is invented for a payload that could not be rendered. The absent arm
/// is reachable only by being called with one, because these payloads are plain
/// data — which is why this is offered rather than kept private.
#[must_use]
pub fn enveloped(status: StatusCode, rendered: Option<String>) -> Response {
    let Some(body) = rendered else {
        return (StatusCode::INTERNAL_SERVER_ERROR, UNRENDERABLE).into_response();
    };
    let mut response = answered(body);
    *response.status_mut() = status;
    response
}

/// A request this surface could not read, said plainly.
///
/// What was asked for is not repeated back. A name lemonfiber does not know is a
/// mistake to correct rather than a request to answer with everything, which is
/// the judgement the command line makes before the core is reached.
fn unreadable(said: &'static str) -> Response {
    (StatusCode::BAD_REQUEST, said).into_response()
}

/// The diagnosis a request asked for, or nothing where it named a group of checks
/// that is not one lemonfiber knows.
fn diagnosis(only: Option<&str>) -> Option<Command> {
    match only {
        None => Some(diagnosing(None)),
        Some(name) => Category::parse(name).map(|group| diagnosing(Some(group))),
    }
}

/// How many existing lines to begin with, or nothing where what was asked for is
/// not a number.
fn counted(said: Option<&str>) -> Option<u32> {
    said.map_or(Some(BEGIN_WITH), |said| said.parse().ok())
}

/// What a request asked for, read from its query string.
///
/// Read here rather than through an extractor, because the router carries no
/// query parser: what this surface takes are the flags the commands take, and a
/// flag a command accepts more than once is a name given more than once.
struct Asked(Vec<(String, String)>);

impl Asked {
    /// The pairs a query string holds, decoded.
    fn read(query: Option<&str>) -> Self {
        let given = query.unwrap_or_default();
        Self(
            form_urlencoded::parse(given.as_bytes())
                .map(|(name, value)| (name.into_owned(), value.into_owned()))
                .collect(),
        )
    }

    /// The first value given for a name, or nothing where it was not given.
    fn one(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(given, _)| given == name)
            .map(|(_, value)| value.as_str())
    }

    /// Every value given for a name, in the order they were given.
    fn every(&self, name: &str) -> Vec<String> {
        self.0
            .iter()
            .filter(|(given, _)| given == name)
            .map(|(_, value)| value.clone())
            .collect()
    }
}
