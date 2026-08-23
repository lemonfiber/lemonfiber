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

use std::net::SocketAddr;

use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, Response, StatusCode};

use crate::guard::{host_is_here, origin_is_here, Token, TOKEN_HEADER};

/// Why a request was not answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// It carried no token, or not this run's.
    Unknown,
    /// It said it came from somewhere this server is not.
    Elsewhere,
}

impl Refusal {
    /// The status a refusal answers with.
    ///
    /// Both are 403 rather than 401: 401 invites a browser to ask for
    /// credentials it has no way to supply, and there is nothing to prompt for.
    #[must_use]
    pub const fn status(self) -> StatusCode {
        StatusCode::FORBIDDEN
    }

    /// What the refusal says, in the one line a reader gets.
    #[must_use]
    pub const fn said(self) -> &'static str {
        match self {
            Self::Unknown => "This request carried no token, or not this run's.",
            Self::Elsewhere => "This request said it came from somewhere this server is not.",
        }
    }
}

/// Whether a request may be answered at all.
///
/// Both checks hold or neither does. The token is what a cross-site request
/// cannot read and therefore cannot send; the address check closes the window a
/// rebound name would open, and neither alone is enough.
///
/// # Errors
///
/// Returns the refusal a caller should answer with.
pub fn admitted(headers: &HeaderMap, token: &Token, bound: SocketAddr) -> Result<(), Refusal> {
    // Looking a header up by name is case-insensitive, so the header may be
    // written here the way the contract prints it.
    let said = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());

    if !token.carried_by(said(TOKEN_HEADER)) {
        return Err(Refusal::Unknown);
    }
    if !host_is_here(said(header::HOST.as_str()), bound)
        || !origin_is_here(said(header::ORIGIN.as_str()), bound)
    {
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
/// A refusal and a request that could not be read are prose, not payloads. They
/// are labelled as prose so that a caller parsing what it was told it was given
/// is not handed a sentence to parse as an envelope.
pub(crate) const SENTENCE: &str = "text/plain; charset=utf-8";

/// The envelope, as the contract states it.
///
/// The body arrives already rendered, because the rendering that answers the
/// command line is the rendering that answers here.
#[must_use]
pub fn answered(rendered: String) -> Response<Body> {
    carrying(StatusCode::OK, JSON, Body::from(rendered))
}

/// A refusal, said plainly rather than as a bare status.
#[must_use]
pub fn refused(refusal: Refusal) -> Response<Body> {
    carrying(refusal.status(), SENTENCE, Body::from(refusal.said()))
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
