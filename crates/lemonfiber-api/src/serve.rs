//! What a request reaches, and what it gets back.
//!
//! The decision a request meets is made here, in one place and without a client
//! or a socket, so that what is refused can be stated as a fact rather than
//! demonstrated by driving a server. The routing beside it is thin on purpose:
//! everything worth testing has already happened by the time a handler runs.
//!
//! No payload is serialised here. An envelope renders itself, and the same
//! rendering answers the command line, so the two cannot say different things
//! about the same state.

use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, Response, StatusCode};

use crate::admission::here;
use crate::guard::{Binding, TOKEN_HEADER};
use crate::refusal::Refusal;

/// Whether a request may be answered at all.
///
/// Both checks hold or neither does. A secret is what a cross-site request cannot
/// read and therefore cannot send; the address check closes the window a rebound
/// name would open, and neither alone is enough.
///
/// Whether the secret is one this run admits is settled before this is called,
/// because there are now two that answer — the token printed at start and a session
/// opened by proving the password — and asking which of them it was is
/// [`crate::admission::Admitting`]'s business rather than this one's. What is left
/// here is the shape of the decision: a secret this run knows, and a request that
/// says it came from where this server is.
///
/// # Errors
///
/// Returns the refusal a caller should answer with.
pub fn admitted(known: bool, headers: &HeaderMap, at: &Binding) -> Result<(), Refusal> {
    if !known {
        return Err(Refusal::NotAdmitted);
    }
    if !here(headers, at) {
        return Err(Refusal::Elsewhere);
    }
    Ok(())
}

/// What an envelope is served as.
pub(crate) const JSON: &str = "application/json";

/// What a stream a client holds open is served as.
pub(crate) const STREAM: &str = "text/event-stream";

/// What a sentence this surface says in its own words is served as.
///
/// Only the page's own absences are said this way — a file the app does not hold,
/// or a build carrying no app — because they answer a browser rather than a
/// client. They are labelled as prose so that nothing parsing what it was told it
/// was given is handed a sentence to parse as an envelope.
pub(crate) const SENTENCE: &str = "text/plain; charset=utf-8";

/// The envelope, as the contract states it.
///
/// The body arrives already rendered, because the rendering that answers the
/// command line is the rendering that answers here.
#[must_use]
pub fn answered(rendered: String) -> Response<Body> {
    carrying(StatusCode::OK, JSON, Body::from(rendered))
}

/// The refusal a member gets at a door that answers the operator alone, or nothing
/// where the caller is not a member.
///
/// For the routes that are not a command and so never reach [`crate::entitled::may`]:
/// a bundle, the logs, the work begun under a job's name and the event stream. Each
/// carries what the operator is shown — a whole support bundle, every container's
/// log lines, another caller's results — and none of it is narrowed to a member, so
/// a member is refused outright rather than handed the operator's copy. A key scoped to
/// a member is that member here as everywhere, and is refused the same way; a `read` or
/// `act` key is admitted, since every served read and the stream are what it is for.
#[must_use]
pub fn operator_only(caller: &crate::admission::Caller) -> Option<Response<Body>> {
    caller
        .member()
        .is_some()
        .then(|| Refusal::NotYours.answered())
}

/// Every response this surface produces, wearing the headers all of them carry.
///
/// One place rather than one per handler: a header a caller's safety rests on is
/// carried by a response having been built here, not by whoever built it having
/// remembered. The bodies differ and the type they are labelled with differs;
/// nothing else about them does.
///
/// Built rather than assembled through a builder: a builder hands back a result
/// whose error arm nothing here can reach, and an arm nothing reaches is one no
/// test can cover.
#[must_use]
pub(crate) fn carrying(status: StatusCode, sort: &'static str, body: Body) -> Response<Body> {
    let mut response = Response::new(body);
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(sort));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

/// The header a caller must send the token in, for a caller building one.
#[must_use]
pub const fn token_header() -> &'static str {
    TOKEN_HEADER
}
