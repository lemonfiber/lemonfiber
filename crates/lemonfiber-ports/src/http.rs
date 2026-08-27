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

use async_trait::async_trait;
use thiserror::Error;

/// The method a request uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Read.
    Get,
    /// Write.
    Post,
    /// Replace an existing resource.
    Put,
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
}

/// What a service answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The status code, whatever it was — a refusal is still an answer.
    pub status: u16,
    /// The body it returned.
    pub body: String,
}

impl Response {
    /// Whether the status is in the 2xx range.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
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
}

impl Unreachable {
    /// A failure nothing retried — the shape a transport reports on its own.
    #[must_use]
    pub fn once(url: &str, reason: &str) -> Self {
        Self {
            url: url.to_owned(),
            reason: reason.to_owned(),
            attempts: 1,
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
}

#[cfg(test)]
mod tests {
    use super::{Method, Request, Response, Unreachable};

    #[test]
    fn a_two_hundred_is_a_success_and_a_refusal_is_not() {
        assert!(Response {
            status: 200,
            body: String::new()
        }
        .is_success());
        assert!(Response {
            status: 204,
            body: String::new()
        }
        .is_success());
        for status in [199, 300, 401, 500] {
            assert!(!Response {
                status,
                body: String::new()
            }
            .is_success());
        }
    }

    #[test]
    fn unreachable_names_the_url_and_the_reason() {
        let failure = Unreachable {
            url: "http://sonarr:8989/api".to_owned(),
            reason: "connection refused".to_owned(),
            attempts: 1,
        };
        let rendered = failure.to_string();
        assert!(rendered.contains("sonarr"));
        assert!(rendered.contains("connection refused"));
    }

    #[test]
    fn a_request_is_plain_data() {
        let request = Request {
            method: Method::Post,
            url: "http://sonarr:8989/api/v3/rootfolder".to_owned(),
            headers: vec![("X-Api-Key".to_owned(), "secret".to_owned())],
            body: Some("{}".to_owned()),
        };
        assert_eq!(request.clone(), request);
        assert_eq!(request.method, Method::Post);
    }
}
