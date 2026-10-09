//! Reaching a service's HTTP API.
//!
//! One seam for everything lemonfiber does over HTTP — proving a credential by
//! reading back an identity, and later wiring one service to another. The
//! transport lives behind this trait so the request-building and
//! response-handling above it run in a test against a fake, and the one crate
//! that speaks HTTP for real stays confined to a single adapter.
//!
//! A status code is never a failure here: a service that answers `401` has
//! answered, and the caller decides what a refusal means. The failure this port
//! reports is the other kind — nothing answered at all.

use std::net::IpAddr;

use async_trait::async_trait;
use thiserror::Error;

/// The media type of a JSON body, asked or answered.
pub const JSON: &str = "application/json";

/// The media type of a refusal written as a problem document.
pub const PROBLEM: &str = "application/problem+json";

/// The method a request uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Read.
    Get,
    /// Write.
    Post,
    /// Replace an existing resource.
    Put,
    /// Change part of an existing resource.
    Patch,
    /// Take a resource away.
    Delete,
}

/// A request to a service's API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Read or write.
    pub method: Method,
    /// The absolute URL.
    pub url: String,
    /// Headers to send, such as the credential the service authenticates with.
    pub headers: Vec<(String, String)>,
    /// The body, where the request carries one.
    pub body: Option<String>,
    /// The only addresses the request may connect to for the host its URL names, where
    /// they were checked before it was sent; nothing where the transport asks the
    /// resolver itself.
    ///
    /// On the request rather than beside it, so a transport that wraps another — one
    /// that retries, one that records — carries it as it carries the rest. Held to, a
    /// name that stands for somewhere else by the time the call is made is not asked
    /// again. The port is the URL's: only the address is pinned.
    pub pinned: Option<Vec<IpAddr>>,
}

/// What a service answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The status code, whatever it was — a refusal is still an answer.
    pub status: u16,
    /// The headers it answered with.
    ///
    /// The mirror of the request's, and it exists for the same reason that one does.
    /// A request may ask for a representation; without this, nothing could see which
    /// one came back — so a caller that asked for a document and was handed an
    /// application shell had no way to tell, and would read the shell as the document.
    /// A service that answers two different things at one path is ordinary rather than
    /// exotic, and the only thing that tells the two answers apart is here.
    ///
    /// A list rather than a map, and in the order they arrived, because a header may
    /// be sent more than once and folding them would make what came back depend on
    /// which copy was read last.
    pub headers: Vec<(String, String)>,
    /// The body it returned.
    pub body: String,
}

impl Response {
    /// Whether the status is in the 2xx range.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// What one header holds, whatever case the service spelled its name in.
    ///
    /// Header names are case-insensitive on the wire and every service spells them
    /// differently, so the matching is done here rather than at each caller — a rule
    /// each of them has to remember is a rule that holds until one of them forgets.
    ///
    /// The first, where a header arrived more than once. Which is the right answer for
    /// the one this is used for: a content type is singular, and a second one is a
    /// service contradicting itself rather than adding to what it said.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        header_in(&self.headers, name)
    }
}

/// What a service answered, read as bytes and no more than a caller would hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// The status code, whatever it was.
    pub status: u16,
    /// The headers it answered with, in the order they arrived.
    pub headers: Vec<(String, String)>,
    /// The body it returned, or nothing where it was larger than was asked for.
    pub bytes: Option<Vec<u8>>,
}

impl Fetched {
    /// Whether the status is in the 2xx range.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// What one header holds, whatever case the service spelled its name in.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        header_in(&self.headers, name)
    }
}

/// The first value of the header `name`, matched without regard to case.
fn header_in<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(held, _)| held.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// The service could not be reached at all.
///
/// Distinct from any status code: this is a refused connection, a name that did
/// not resolve, a handshake that did not complete, or a wait that ran out —
/// nothing answered, so nothing can be concluded about the credential.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("{url} could not be reached: {reason}")]
pub struct Unreachable {
    /// The URL that was tried.
    pub url: String,
    /// The transport's own account of why. Theirs verbatim — what lemonfiber
    /// makes of it belongs beside it, never rewritten over it.
    pub reason: String,
    /// How many times it was tried before giving up, which is what separates a
    /// service that was busy from one that is down. One where nothing retried it.
    pub attempts: u32,
    /// Whether a connection was made before it failed.
    ///
    /// A service that took the connection and then said nothing within the wait is
    /// not a blip a moment's pause cures: asking it again costs the whole wait
    /// again. Only a connection that was never made is worth trying again.
    pub connected: bool,
}

impl Unreachable {
    /// A failure nothing retried — the shape a transport reports on its own.
    #[must_use]
    pub fn once(url: &str, reason: &str) -> Self {
        Self {
            url: url.to_owned(),
            reason: reason.to_owned(),
            attempts: 1,
            connected: false,
        }
    }
}

/// Sends HTTP requests to services and returns what they answered.
#[async_trait]
pub trait Http: Send + Sync {
    /// Send a request and read the response.
    ///
    /// # Errors
    ///
    /// Returns [`Unreachable`] when nothing answered. A status code — including a
    /// refusal — is a [`Response`], never an error.
    async fn send(&self, request: &Request) -> Result<Response, Unreachable>;

    /// Send a request and read the body as bytes, holding no more than `most` of them.
    ///
    /// # Errors
    ///
    /// Returns [`Unreachable`] when nothing answered.
    async fn fetch(&self, request: &Request, most: usize) -> Result<Fetched, Unreachable> {
        self.send(request).await.map(|response| Fetched {
            status: response.status,
            headers: response.headers,
            bytes: Some(response.body.into_bytes()).filter(|bytes| bytes.len() <= most),
        })
    }
}

/// A shared handle to a transport is a transport.
///
/// Needed wherever something wraps what is already being shared — a decorator that
/// records or retries takes what it wraps, and by the time one is built the
/// transport is usually behind a handle already. Stated here rather than worked
/// around at each wrapping, because it is a fact about the port and not about any
/// one adapter.
#[async_trait]
impl Http for std::sync::Arc<dyn Http> {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        (**self).send(request).await
    }

    async fn fetch(&self, request: &Request, most: usize) -> Result<Fetched, Unreachable> {
        (**self).fetch(request, most).await
    }
}

#[cfg(test)]
mod tests;
