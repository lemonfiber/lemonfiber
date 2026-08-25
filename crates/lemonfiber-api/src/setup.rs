//! First-run setup, one request at a time.
//!
//! A terminal holds the whole conversation on its stack; a browser cannot, so the
//! walk is taken a request at a time. Each one names a step of it — where am I,
//! here is one answer, back one, apply — and each becomes one of the core's own
//! commands, so what a browser can do to a fresh machine is what the command line
//! can do to it and nothing more.
//!
//! Nothing is held between requests here. The answers gathered so far live in the
//! resumable progress file setup already writes, which is what a terminal run
//! reads too: a setup begun in a browser is one a terminal finishes, two windows
//! open on it are looking at one run, and reloading the page loses nothing.
//!
//! Its own endpoints rather than named actions, because the actions surface takes
//! one flat carrier of arguments and an answer is not one — it is the question it
//! belongs to, tagged, and a flat carrier would let one arrive answering two.
//!
//! **What comes back never repeats what was entered.** Setup gathers an indexer
//! key and a provider password, and a machine-readable answer is one a caller can
//! log; the report says what was decided and withholds every value nobody has
//! written down a reason for showing, exactly as `config show` does.

use axum::body::Body;
use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use lemonfiber_core::app::{dispatch, Command, SetupAction};
use lemonfiber_core::model::{kind, Envelope};
use lemonfiber_core::wizard::Answer;

use crate::read::{enveloped, refusing};
use crate::router::Serving;
use crate::serve::{carrying, SENTENCE};

/// What is said to a request whose body is not an answer.
const NOT_AN_ANSWER: &str = "The body of this request is not one of setup's answers.";

/// The five requests setup is walked with.
pub fn routes() -> Router<Serving> {
    Router::new()
        .route("/api/setup", get(standing))
        .route("/api/setup/answer", post(answered))
        .route("/api/setup/next", post(onward))
        .route("/api/setup/back", post(backward))
        .route("/api/setup/apply", post(applied))
}

/// Where setup stands, and what it is still asking for.
async fn standing(State(serving): State<Serving>) -> Response {
    walked(&serving, SetupAction::Where).await
}

/// One question answered.
async fn answered(
    State(serving): State<Serving>,
    given: Result<Json<Answer>, JsonRejection>,
) -> Response {
    let Ok(Json(answer)) = given else {
        return unreadable();
    };
    walked(&serving, SetupAction::Answer(answer)).await
}

/// On to the next step, for the steps that only inform.
async fn onward(State(serving): State<Serving>) -> Response {
    walked(&serving, SetupAction::Next).await
}

/// Back to the previous question.
async fn backward(State(serving): State<Serving>) -> Response {
    walked(&serving, SetupAction::Back).await
}

/// The reviewed answers written.
///
/// Answered with its outcome rather than with a name to follow, because applying
/// reads and writes lemonfiber's own files and reaches neither the container
/// engine nor a service — it has finished by the time a reply could be written.
async fn applied(State(serving): State<Serving>) -> Response {
    walked(&serving, SetupAction::Apply).await
}

/// One step of the walk, carried out and answered with where it left setup.
async fn walked(serving: &Serving, action: SetupAction) -> Response {
    match dispatch(Command::Setup(action), &serving.ctx).await {
        Ok(outcome) => enveloped(StatusCode::OK, outcome.envelope().to_json()),
        Err(problem) => enveloped(
            refusing(&problem),
            Envelope::new(kind::ERROR, &*problem).to_json(),
        ),
    }
}

/// A body this surface could not read, said plainly.
///
/// What arrived is not quoted back. An answer carries a credential, and a message
/// repeating the body would carry it wherever the message goes.
fn unreadable() -> Response {
    carrying(StatusCode::BAD_REQUEST, SENTENCE, Body::from(NOT_AN_ANSWER))
}
