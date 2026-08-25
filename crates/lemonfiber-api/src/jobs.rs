//! Work that outlives the request that started it, and how it is asked about.
//!
//! An action reaching the container engine or a service runs for minutes, so it is
//! not waited for. The request is answered with a name and the work is handed to
//! the runtime, where whatever becomes of the request — answered, abandoned, a
//! browser tab closed — cannot reach it.
//!
//! A name is only an answer if it can be redeemed, so this is both halves: the
//! name minted and handed over, and the endpoint that says what became of the work
//! it names. Without the second half a caller holds a word and nothing to do with
//! it, and a reply that is only a word is then not an answer at all.
//!
//! What became of the work is said by the status. Still going is the same body the
//! request that started it was given, under the same status, because there is
//! nothing more to say until there is an outcome; finished is the envelope the
//! equivalent command renders, byte for byte, and stopped is the envelope the
//! failure renders. So a caller parses one document either way and reads the status
//! to know which of the two it has.
//!
//! Nothing outlives the run. A job names work in flight, work in flight does not
//! survive the process doing it, and a record that did would describe jobs nothing
//! is running.

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use lemonfiber_core::app::{dispatch, Command, Ctx};
use lemonfiber_core::model::{kind, Envelope, Started};
use lemonfiber_core::ports::random::Random;
use tokio::sync::Mutex;

use crate::guard::hex;
use crate::read::enveloped;
use crate::router::Serving;
use crate::serve::{carrying, SENTENCE};

/// Bytes of name. Wide enough that two runs never mint the same one.
const WIDTH: usize = 8;

/// What is said about a name this run never handed out.
///
/// What was asked for is not repeated back, and nothing distinguishes a name that
/// was never minted from one another run minted: this run knows only its own.
const NO_SUCH_JOB: &str = "No work in this run goes by that name.";

/// A name for work that outlives the request that started it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job(String);

impl Job {
    /// Mints one, or nothing when the operating system will not say.
    ///
    /// Through the port rather than taken directly, so a test names a job it
    /// chose instead of depending on what the machine happens to produce.
    pub fn mint(random: &dyn Random) -> Option<Self> {
        Some(Self(hex(&random.bytes(WIDTH)?)))
    }

    /// The name as it is answered with, and as it is asked about afterwards.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Where a piece of work got to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// Still going.
    Running,
    /// Finished, and this is the envelope it came to.
    Done(String),
    /// Stopped, and this is the envelope saying why.
    Failed(String),
}

/// One piece of work this run started: what was asked for, and where it got to.
///
/// The action is kept beside the standing because a caller asking about a name is
/// answered with what that name was for. It already told this surface once, and
/// asking it to remember which name it gave which request is asking it to keep a
/// second copy of what this already knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Work {
    /// The action that was asked for, as it was named.
    pub action: String,
    /// Where it got to.
    pub standing: Standing,
}

/// The work this run started, and where each piece of it got to.
///
/// Held for the life of the run rather than written down. A job names work in
/// flight, and work in flight does not survive the process that is doing it —
/// so a record that outlived the run would describe jobs nothing is running.
#[derive(Clone, Default)]
pub struct Jobs(Arc<Mutex<HashMap<String, Work>>>);

impl Jobs {
    /// Start work under a name, and stop holding on to it.
    ///
    /// The work is handed to the runtime rather than awaited, which is the whole
    /// point: what happens to the request afterwards — answered, abandoned, a tab
    /// closed — cannot reach it.
    pub async fn start(&self, job: &Job, action: &str, command: Command, ctx: Arc<Ctx>) {
        let (name, held) = (job.as_str().to_owned(), Arc::clone(&self.0));
        let action = action.to_owned();
        held.lock().await.insert(
            name.clone(),
            Work {
                action: action.clone(),
                standing: Standing::Running,
            },
        );
        tokio::spawn(async move {
            // Nothing is invented for a payload that could not be rendered: these
            // are plain owned values, and the empty arm is reached only by being
            // handed one, which nothing here can be.
            let standing = match dispatch(command, &ctx).await {
                Ok(outcome) => Standing::Done(outcome.envelope().to_json().unwrap_or_default()),
                Err(problem) => Standing::Failed(
                    Envelope::new(kind::ERROR, &*problem)
                        .to_json()
                        .unwrap_or_default(),
                ),
            };
            held.lock().await.insert(name, Work { action, standing });
        });
    }

    /// The work a name stands for, or nothing for a name this run never handed out.
    pub async fn about(&self, job: &str) -> Option<Work> {
        self.0.lock().await.get(job).cloned()
    }
}

/// The one thing that can be asked about work already begun.
pub fn routes() -> Router<Serving> {
    Router::new().route("/api/jobs/{job}", get(became))
}

/// What became of the work one name stands for.
///
/// A name this run never handed out is absent rather than reported as unfinished:
/// answering "still going" for work nothing is doing would leave a caller waiting
/// on an outcome that is never coming.
async fn became(State(serving): State<Serving>, Path(job): Path<String>) -> Response {
    let Some(work) = serving.jobs.about(&job).await else {
        return unknown();
    };
    match work.standing {
        Standing::Running => still(&job, &work.action),
        Standing::Done(rendered) => enveloped(StatusCode::OK, Some(rendered)),
        // The status a command that could not be carried out is answered with
        // everywhere on this surface. The body is still the envelope, because a
        // caller that asked for something it could parse asked about the failures
        // most of all.
        Standing::Failed(rendered) => enveloped(StatusCode::INTERNAL_SERVER_ERROR, Some(rendered)),
    }
}

/// A job accepted, named so that what became of it can be asked.
#[must_use]
pub fn accepted(job: &Job, action: &str) -> Response {
    still(job.as_str(), action)
}

/// The name and the action, under the status that means the work is not finished.
///
/// One body for two moments. The request that starts work and a request asking
/// about work still going have the same thing to be told — that there is a name
/// and what it was for — so a caller that can read the first can read the second
/// without learning a second shape.
fn still(job: &str, action: &str) -> Response {
    let started = Started {
        job: job.to_owned(),
        action: action.to_owned(),
    };
    enveloped(
        StatusCode::ACCEPTED,
        Envelope::new(kind::JOB, started).to_json(),
    )
}

/// A name this run never handed out, said plainly.
fn unknown() -> Response {
    carrying(StatusCode::NOT_FOUND, SENTENCE, Body::from(NO_SUCH_JOB))
}
