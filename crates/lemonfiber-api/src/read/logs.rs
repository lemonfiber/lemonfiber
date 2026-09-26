//! What the services are saying, the one read of the stack that is not a command's
//! envelope: it may stay open and hand on each line as it arrives.

use axum::extract::{RawQuery, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use lemonfiber_core::app::{logs, Ctx};
use lemonfiber_core::model::{kind, Envelope};
use lemonfiber_core::ports::docker::{LogLine, LogQuery};

use crate::admission::Caller;
use crate::read::table::{Asked, FOLLOW, FORM, LOGS, SERVICE, TAIL};
use crate::router::Serving;
use crate::serve::operator_only;

use super::{enveloped, unreadable, went_wrong};

/// The most existing lines this read will begin with.
///
/// The command line hands each line on as it arrives and keeps none of them. This
/// read has no last element to close a document with, so it gathers the whole
/// scrollback before it answers any of it — which makes the number a caller writes
/// here the number of lines this machine holds in memory at once. A ceiling is on
/// the reading of it rather than on the gathering, because the honest answer to a
/// request for four billion lines is that it will not be answered, and quietly
/// gathering fewer would answer a different request.
const AT_MOST: u32 = 10_000;

/// What is said to a request whose line count is not a number, or is past the
/// ceiling. One sentence, because both are the same mistake about the same word.
fn not_a_count() -> String {
    format!("How many lines to begin with must be a number, and no more than {AT_MOST}.")
}

/// What is said to a request whose follow is neither yes nor no.
const NOT_A_CHOICE: &str = "Whether to keep reading must be true or false.";

/// The read of what the services are saying.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(LOGS, get(log_lines))
}

/// What the services are saying, and — where it is asked for — what they say next.
///
/// One endpoint over one request, because the command line spells it as one:
/// following is a flag on `logs` rather than a request of its own, and the lines
/// it reads are the lines the scrollback reads. What differs is that it does not
/// end, so it cannot be answered with what it read. It is answered with a name for
/// the work instead — the same answer every request that outlives its own
/// connection gets here — and the lines arrive on the stream.
async fn log_lines(
    State(serving): State<Serving>,
    caller: Caller,
    RawQuery(query): RawQuery,
) -> Response {
    if let Some(refusal) = operator_only(&caller) {
        return refusal;
    }
    let asked = match Asked::read(LOGS, query.as_deref()) {
        Ok(asked) => asked,
        Err(problem) => return went_wrong(&problem),
    };
    let Some(tail) = counted(asked.one(TAIL)) else {
        return unreadable(&not_a_count());
    };
    let Some(follow) = told(asked.one(FOLLOW)) else {
        return unreadable(NOT_A_CHOICE);
    };
    let (forms, services) = (asked.every(FORM), asked.every(SERVICE));
    if follow {
        return crate::following::followed(&serving, forms, services, tail).await;
    }
    read_logs(&serving.ctx, &forms, &services, tail).await
}

/// Whether a request asked to keep reading, or nothing where it said something
/// that is neither.
///
/// Not given is not asked for. A word that is neither is a mistake to correct
/// rather than a request to answer with the scrollback, because the two answers
/// are different shapes — lines, or a name — and a caller that meant to follow
/// would otherwise parse an answer it never asked for.
fn told(said: Option<&str>) -> Option<bool> {
    match said {
        // Not given is not asked for, which is the same answer as having said so.
        None | Some("false") => Some(false),
        Some("true") => Some(true),
        Some(_) => None,
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

/// How many existing lines to begin with, or nothing where what was asked for is
/// not a number or is more than this read will gather.
fn counted(said: Option<&str>) -> Option<u32> {
    // The command line's own number, read from where it is decided rather than
    // restated here, so the two really do answer a silent request with the same
    // lines instead of only saying they do.
    said.map_or(Some(LogQuery::BEGIN_WITH), |said| {
        said.parse().ok().filter(|count| *count <= AT_MOST)
    })
}
