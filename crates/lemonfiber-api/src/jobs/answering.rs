//! The job routes, and how where a piece of work stands is answered over them.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use lemonfiber_core::model::kind::{Kind, ENDED, JOB};
use lemonfiber_core::model::Envelope;

use super::ended::Ended;
use super::started::Started;
use super::{Job, Standing, Work, A_JOB, FORGET};
use crate::admission::Caller;
use crate::read::enveloped;
use crate::refusal::Refusal;
use crate::router::Serving;
use crate::routing::{Serves, Taking};

/// What can be asked about work already begun, and the two things that can be done
/// to it, each answering with where the work stands: still going, what its command came
/// to, or how it ended.
#[must_use]
pub fn routes() -> Serves<Serving> {
    let standing = standing();
    Serves::new()
        .get(A_JOB, became, &standing)
        .delete(A_JOB, released, Taking::Nothing, &standing)
        .post(FORGET, forgotten, Taking::Nothing, &standing)
}

/// Every kind where a piece of work stands may be said under.
fn standing() -> Vec<Kind> {
    let mut kinds = crate::actions::answered_under();
    kinds.push(ENDED);
    kinds.sort_by_key(|kind| kind.as_str());
    kinds.dedup();
    kinds
}

/// What became of the work one name stands for.
///
/// A name this run never handed out is absent rather than reported as unfinished:
/// answering "still going" for work nothing is doing would leave a caller waiting
/// on an outcome that is never coming.
async fn became(
    State(serving): State<Serving>,
    caller: Caller,
    Path(job): Path<String>,
) -> Response {
    answered(&job, serving.jobs.about(&job, &caller).await)
}

/// The work one name stands for, ended.
///
/// The same answer asking would have given, because releasing a name and asking
/// about it are two questions with one answer: where the work now stands. So a
/// caller that released one need not ask again to find out what it released.
async fn released(
    State(serving): State<Serving>,
    caller: Caller,
    Path(job): Path<String>,
) -> Response {
    answered(&job, serving.jobs.stop(&job, &caller).await)
}

/// One name let go of, the work it stands for left running.
async fn forgotten(
    State(serving): State<Serving>,
    caller: Caller,
    Path(job): Path<String>,
) -> Response {
    answered(&job, serving.jobs.forget(&job, &caller).await)
}

/// Where the work a name stands for stands, or the refusal of a name with nothing
/// this caller may be told of behind it.
fn answered(job: &str, work: Option<Work>) -> Response {
    work.map_or_else(unknown, |work| stands(job, &work))
}

/// Where a piece of work stands, as the answer a caller reads.
fn stands(job: &str, work: &Work) -> Response {
    match &work.standing {
        Standing::Running => still(job, &work.action),
        Standing::Done(rendered) => enveloped(StatusCode::OK, Some(rendered.clone())),
        // The status the refusal itself warrants, decided where the problem was
        // still in hand rather than again here. A form nothing declares is the same
        // mistake whether it was named to a read or to an action redeemed by name,
        // and answering the second `500` sends a caller on retrying what cannot
        // succeed. The body is still the envelope, because a caller that asked for
        // something it could parse asked about the failures most of all.
        Standing::Failed(rendered, status) => enveloped(*status, Some(rendered.clone())),
        Standing::Ended(why) => {
            let ended = Ended {
                job: job.to_owned(),
                action: work.action.clone(),
                ended: *why,
            };
            enveloped(StatusCode::OK, Envelope::new(ENDED, ended).to_json())
        }
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
    enveloped(StatusCode::ACCEPTED, Envelope::new(JOB, started).to_json())
}

/// A name this run never handed out.
fn unknown() -> Response {
    Refusal::NoSuchJob.answered()
}
