//! The plumbing every service client shares over the HTTP port.
//!
//! Servarr and qBittorrent speak different APIs, but they reach them the same
//! way: a base address, a name to blame a failure on, and one reading of what a
//! transport error or a non-success status means. That common part lives here so
//! it is written once; each client builds its own requests and holds one of these
//! to send them.

use std::sync::Arc;

use crate::ports::http::{Http, Request, Response};
use crate::ports::service::Failure;

/// A service reached over the HTTP port: where it is, and what to call it when
/// something goes wrong.
pub(crate) struct Endpoint {
    http: Arc<dyn Http>,
    base: String,
    service: String,
}

impl Endpoint {
    /// An endpoint for `service`, reached at `base` over `http`.
    pub(crate) fn new(
        http: Arc<dyn Http>,
        base: impl Into<String>,
        service: impl Into<String>,
    ) -> Self {
        Self {
            http,
            base: base.into(),
            service: service.into(),
        }
    }

    /// A URL for `path` under this endpoint's base, with any trailing slash on
    /// the base dropped so the join never doubles it.
    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base.trim_end_matches('/'))
    }

    /// Send a request, turning a transport failure into "not answering".
    ///
    /// The port reports the no-answer case; here it becomes the service failure
    /// the rest of the system acts on — a prerequisite that is not up yet, to be
    /// skipped and picked up on a later run rather than counted as broken.
    pub(crate) async fn send(&self, request: &Request) -> Result<Response, Failure> {
        self.http
            .send(request)
            .await
            .map_err(|_| Failure::Unavailable {
                service: self.service.clone(),
            })
    }

    /// What a non-success response amounts to: a refused credential, or an answer
    /// lemonfiber cannot use, carrying the service's own words.
    pub(crate) fn refusal(&self, response: &Response) -> Failure {
        match response.status {
            401 | 403 => self.unauthorised(),
            _ => self.refused(&describe(response)),
        }
    }

    /// A rejected credential, blamed on this service.
    pub(crate) fn unauthorised(&self) -> Failure {
        Failure::Unauthorised {
            service: self.service.clone(),
        }
    }

    /// A refusal this endpoint blames on itself, with a caller-supplied reason —
    /// for an answer that arrived but could not be used.
    pub(crate) fn refused(&self, detail: &str) -> Failure {
        Failure::Refused {
            service: self.service.clone(),
            detail: detail.to_owned(),
        }
    }

    /// Read a JSON body into `T`, or fail: a non-success status is the service's
    /// refusal, and a body that will not parse is an answer that arrived but
    /// could not be used, named by `what`.
    ///
    /// The parser's own account of what failed is kept, not paraphrased: when a
    /// pinned service's release changes the shape of a response, "missing field
    /// `version`" names the break where a generic sentence would hide it. Every
    /// client that reads a list back shares this, so the reading is written once.
    pub(crate) fn decode<T: serde::de::DeserializeOwned>(
        &self,
        response: &Response,
        what: &str,
    ) -> Result<T, Failure> {
        if !response.is_success() {
            return Err(self.refusal(response));
        }
        serde_json::from_str(&response.body).map_err(|err| self.refused(&format!("{what}: {err}")))
    }
}

/// A non-success response as the detail of a refusal: the service's own words
/// where it gave any, its status code alone otherwise.
pub(crate) fn describe(response: &Response) -> String {
    let body = response.body.trim();
    if body.is_empty() {
        format!("HTTP {}", response.status)
    } else {
        format!("HTTP {}: {body}", response.status)
    }
}
