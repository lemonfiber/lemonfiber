//! The reads: one endpoint per question a command already answers, plus
//! the two that answer with something other than a value.
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
//! Which command a read reaches is [`crate::read::table`]'s, named by the path it is
//! served at, so another surface can ask for the same read by the same name and
//! reach the same command. What a read takes is named there too, beside the
//! command, so one place refuses a parameter no read takes — including on the
//! reads that take nothing at all. What is left here is the carrying out: one route
//! per name in that table, and the two reads that answer with a stream and a file.

mod bundle;
mod logs;
pub mod table;

use axum::body::Body;
use axum::extract::{RawQuery, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use lemonfiber_core::app::{dispatch, Command, Ctx};
use lemonfiber_core::error::{Amiss, Problem};
use lemonfiber_core::model::{kind, Envelope};

use crate::admission::Caller;
use crate::entitled::{may, Permitted};
use crate::read::table::{named, wanted, OFFERED};
use crate::router::Serving;
use crate::serve::{answered, carrying, refused, Refusal, SENTENCE};

/// The status a read that this machine could not answer is refused with.
///
/// The body is still the envelope, because a caller that asked for something it
/// could parse asked about the failures most of all.
const FAILED: StatusCode = StatusCode::INTERNAL_SERVER_ERROR;

/// What is said where a payload could not be rendered.
const UNRENDERABLE: &str = "This answer could not be rendered.";

/// The reads this surface answers: every read [`OFFERED`] names, each carried out by
/// [`reading`] under its own name, and the two that answer with something other than
/// an envelope.
pub fn routes() -> Router<Serving> {
    OFFERED
        .iter()
        .fold(Router::new(), |router, &read| {
            router.route(
                read,
                get(
                    move |State(serving): State<Serving>,
                          caller: Caller,
                          RawQuery(query): RawQuery| async move {
                        reading(&serving.ctx, &caller, read, query.as_deref()).await
                    },
                ),
            )
        })
        .merge(logs::routes())
        .merge(bundle::routes())
}

/// Carry out the read a name reaches, or say why it cannot be.
///
/// Every endpoint below arrives here with its own name and the query string as it
/// arrived, so the name a path is served under, what may be said alongside it and
/// the command it comes to are one decision made in one place rather than one
/// made per route.
pub(crate) async fn reading(
    ctx: &Ctx,
    caller: &Caller,
    read: &str,
    query: Option<&str>,
) -> Response {
    let given = match wanted(read, query) {
        Ok(given) => given,
        Err(problem) => return went_wrong(&problem),
    };
    match named(read, given) {
        // Ruled on between naming the command and carrying it out, so what is
        // carried out is what this caller may have — narrowed where they may have
        // part of it, and nothing where it is not theirs at all.
        Ok(command) => match may(caller, command) {
            Permitted::This(command) => carried_out(ctx, command).await,
            Permitted::Nothing => refused(Refusal::NotYours),
        },
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
