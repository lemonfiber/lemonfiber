//! The twenty reads: one endpoint per question a command already answers, plus the
//! two that answer with something other than a value.
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
//! Which command a read reaches is [`crate::reads`]'s, named by the path it is
//! served at, so another surface can ask for the same read by the same name and
//! reach the same command. What a read takes is named there too, beside the
//! command, so one place refuses a parameter no read takes — including on the
//! reads that take nothing at all. What is left here is the carrying out. The
//! endpoints themselves are grouped beside it by what they are about: the stack,
//! the diagnosis, one item, where the household begins, the choices in force, the
//! words, the files lemonfiber keeps of its own, and where the disk went.

mod archives;
mod chosen;
mod clients;
mod diagnosis;
mod door;
mod glossary;
mod items;
mod outbound;
mod space;
mod stack;
mod stored;

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;
use axum::Router;
use lemonfiber_core::app::{dispatch, Command, Ctx};
use lemonfiber_core::error::{Amiss, Problem};
use lemonfiber_core::model::{kind, Envelope};

use crate::reads::{named, wanted};
use crate::router::Serving;
use crate::serve::{answered, carrying, SENTENCE};

/// The status a read that this machine could not answer is refused with.
///
/// The body is still the envelope, because a caller that asked for something it
/// could parse asked about the failures most of all.
const FAILED: StatusCode = StatusCode::INTERNAL_SERVER_ERROR;

/// What is said where a payload could not be rendered.
const UNRENDERABLE: &str = "This answer could not be rendered.";

/// The reads this surface answers.
pub fn routes() -> Router<Serving> {
    Router::new()
        .merge(stack::routes())
        .merge(diagnosis::routes())
        .merge(items::routes())
        .merge(door::routes())
        .merge(chosen::routes())
        .merge(glossary::routes())
        .merge(archives::routes())
        .merge(clients::routes())
        .merge(outbound::routes())
        .merge(stored::routes())
        .merge(space::routes())
}

/// Carry out the read a name reaches, or say why it cannot be.
///
/// Every endpoint below arrives here with its own name and the query string as it
/// arrived, so the name a path is served under, what may be said alongside it and
/// the command it comes to are one decision made in one place rather than
/// thirteen made per handler.
pub(crate) async fn reading(ctx: &Ctx, read: &str, query: Option<&str>) -> Response {
    let given = match wanted(read, query) {
        Ok(given) => given,
        Err(problem) => return went_wrong(&problem),
    };
    match named(read, given) {
        Ok(command) => carried_out(ctx, command).await,
        Err(said) => unreadable(said),
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

/// The failure a command reported, in the envelope machine-readable output gives
/// it, at the status the refusal warrants.
pub(crate) fn went_wrong(problem: &Problem) -> Response {
    enveloped(
        refusing(problem),
        Envelope::new(kind::ERROR, problem).to_json(),
    )
}

/// The status a refusal warrants.
///
/// The body is the envelope whichever it is — a caller that asked for something it
/// could parse asked about the refusals most of all — so the status is the only
/// thing that tells them apart, and they are worth telling apart: a browser
/// answered 500 for a word this product does not explain would go on retrying
/// what cannot succeed, and would have to word its message so as to be true of a
/// broken stack as well.
///
/// Read from the problem rather than decided here. Which of these a code means is
/// known where the code is raised and nowhere else; a list of codes kept on this
/// side would be a second place to remember, and a code added later would answer
/// wrongly until somebody thought to come back.
///
/// The two a caller can act on are told apart the way the write surface tells its
/// own apart: what a request *named* and this product does not have is absent,
/// and how a request *asked* is bad. Every surface that answers with a problem
/// reads this one, so a single refusal cannot carry two statuses depending on
/// which door it arrived through.
pub(crate) const fn refusing(problem: &Problem) -> StatusCode {
    match problem.amiss {
        Amiss::Naming => StatusCode::NOT_FOUND,
        Amiss::Asking => StatusCode::BAD_REQUEST,
        Amiss::Answering => FAILED,
    }
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
        return carrying(
            StatusCode::INTERNAL_SERVER_ERROR,
            SENTENCE,
            Body::from(UNRENDERABLE),
        );
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
pub(crate) fn unreadable(said: &str) -> Response {
    carrying(
        StatusCode::BAD_REQUEST,
        SENTENCE,
        Body::from(said.to_owned()),
    )
}
