//! What a write request asks for, and what becomes of it.
//!
//! An action is named, the name is turned into one of the core's own commands, and
//! the command is carried out or handed to the runtime. Which name reaches which
//! command lives in [`named`], what a caller may say alongside it in [`asked`], and
//! why one was turned away in [`refused`]; what is here is the route itself and the
//! one decision that belongs to neither — when the answer arrives.
//!
//! An action that reaches the container engine or the services runs for minutes,
//! and a request that waited for it would tie the work to a connection. So those
//! are answered with a name for the work instead, and the work runs somewhere the
//! connection cannot reach — a browser tab closed mid-repair takes nothing with
//! it. What that name is redeemed for lives in [`crate::jobs`], and so does the
//! other thing a name is for: a browser has no interruption to send, so releasing
//! the name is how an action that would otherwise run all afternoon is stopped. An
//! action that only reads and writes lemonfiber's own files is answered with its
//! outcome, because it has already finished by the time a reply could be.
//!
//! No payload is serialised here. An envelope renders itself, and the same
//! rendering answers the command line.

mod asked;
mod named;
mod refused;

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::post;
use axum::{Json, Router};
use lemonfiber_core::app::restore::Consent as RestoreConsent;
use lemonfiber_core::app::Command;

use crate::jobs::{accepted, Job};
use crate::read::carried_out;
use crate::router::Serving;
use crate::serve::{carrying, SENTENCE};

pub use asked::{
    Arguments, Disturbing, TAKES_AGREED, TAKES_AGREEMENT, TAKES_ALLOWANCE, TAKES_ARCHIVE,
    TAKES_BUNDLING, TAKES_CHECK, TAKES_CONSENT, TAKES_DISRUPTION, TAKES_DOWNLOAD, TAKES_FORMS,
    TAKES_ITEM, TAKES_NAME, TAKES_NARROWING, TAKES_POLICY, TAKES_PRESET, TAKES_REASON,
    TAKES_REQUEST, TAKES_SERVICE, TAKES_SERVICES, TAKES_SETTING, TAKES_TERM, TAKES_WAITING,
};
pub use named::{named, OFFERED};
pub use refused::Refused;

/// When an action's answer arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answering {
    /// With the outcome, because the work is already done.
    Now,
    /// With a name for the work, which goes on reporting after this reply.
    Later,
}

/// Which of the two an action is.
///
/// The rule is where the work happens rather than how long it has taken before:
/// anything reaching the container engine or a service is a wait an operator
/// should be able to watch, and anything confined to lemonfiber's own files has
/// finished by the time a reply could be written.
///
/// A restore is both, and which it is turns on the one field that says whether it
/// writes. Unconfirmed it reads an archive's own account of itself and touches
/// nothing, which is the listing an operator is owed *before* deciding — so it
/// arrives now, because a listing behind a job name is a listing that arrives after
/// the moment it exists for.
#[must_use]
pub const fn answering(command: &Command) -> Answering {
    match command {
        Command::ConfigSet { .. }
        | Command::Quality(_)
        | Command::Setup(_)
        | Command::Restore {
            consent: RestoreConsent::List,
            ..
        } => Answering::Now,
        _ => Answering::Later,
    }
}

/// An action refused, said plainly rather than as a bare status.
///
/// Prose rather than an envelope, and labelled as prose, the way every other
/// request this surface could not read is answered.
#[must_use]
pub fn declined(refused: &Refused) -> Response {
    carrying(refused.status(), SENTENCE, Body::from(refused.said()))
}

/// The route every action is asked for through.
///
/// Admission is not applied here. Whether a request may be answered at all is one
/// question for the whole surface, asked once above the whole tree, which is what
/// keeps an endpoint added later from arriving unguarded.
pub fn routes() -> Router<Serving> {
    Router::new().route("/api/actions/{action}", post(taken))
}

/// One action, carried out or refused.
async fn taken(
    State(serving): State<Serving>,
    Path(action): Path<String>,
    Json(given): Json<Arguments>,
) -> Response {
    let command = match named(&action, given) {
        Ok(command) => command,
        Err(refused) => return declined(&refused),
    };
    match answering(&command) {
        Answering::Now => carried_out(&serving.ctx, command).await,
        Answering::Later => {
            let Some(job) = Job::mint(serving.ctx.random.as_ref()) else {
                return unnameable();
            };
            serving
                .jobs
                .start(&job, &action, command, Arc::clone(&serving.ctx))
                .await;
            accepted(&job, &action)
        }
    }
}

/// Work that could not be named, and therefore was not begun.
///
/// A job with no name is work nothing could ever be told about, so there is
/// nothing here to fall back to. Shared with the one long-running request that is
/// asked for as a read, because a name it cannot mint stops it in the same way.
pub(crate) fn unnameable() -> Response {
    carrying(
        StatusCode::INTERNAL_SERVER_ERROR,
        SENTENCE,
        Body::from("This machine would not supply the randomness a job needs to be named."),
    )
}
