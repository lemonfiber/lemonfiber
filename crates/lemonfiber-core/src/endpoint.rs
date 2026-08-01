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

/// The header a Servarr-shape service authenticates with — Sonarr, Radarr,
/// Lidarr and Prowlarr all read the key from it, and the setup validator sends
/// it the same way, so its name is written once.
pub(crate) const API_KEY_HEADER: &str = "X-Api-Key";

/// The media type a JSON request declares, so a service binds the body rather
/// than refusing it.
const JSON: &str = "application/json";

/// The `Content-Type` header a JSON request carries: present only where the
/// request has a body, absent on a bodiless GET. Returned as an `Option` so a
/// caller folds it into its header list with `extend`.
pub(crate) fn json_content_type(body: Option<&String>) -> Option<(String, String)> {
    body.map(|_| ("Content-Type".to_owned(), JSON.to_owned()))
}

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

    /// The service does not serve the API version this build speaks — the reason
    /// a versioned endpoint answered `404`. Reported rather than written to.
    pub(crate) fn unsupported(&self, detail: &str) -> Failure {
        Failure::Unsupported {
            service: self.service.clone(),
            detail: detail.to_owned(),
        }
    }

    /// Nothing when the response succeeded, the refusal a non-success status
    /// amounts to otherwise — the answer to "did it take?" for a call whose body
    /// the caller discards. Used with `?` it also serves as a guard partway
    /// through a longer exchange.
    pub(crate) fn expect_success(&self, response: &Response) -> Result<(), Failure> {
        if response.is_success() {
            Ok(())
        } else {
            Err(self.refusal(response))
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

/// The most of a service's error body carried into a refusal — enough to diagnose,
/// capped so a large or verbose response (which could echo a field the request
/// submitted) is not dumped wholesale into operator-facing output.
const DETAIL_LIMIT: usize = 200;

/// A non-success response as the detail of a refusal: the service's own words
/// where it gave any — clipped to a diagnostic length — its status code alone
/// otherwise.
pub(crate) fn describe(response: &Response) -> String {
    let body = response.body.trim();
    if body.is_empty() {
        format!("HTTP {}", response.status)
    } else {
        format!("HTTP {}: {}", response.status, clip(body, DETAIL_LIMIT))
    }
}

/// `text` cut to at most `limit` characters — counted as characters, not bytes, so
/// a multibyte boundary is never split — and marked where it was cut.
fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let kept: String = text.chars().take(limit).collect();
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use super::describe;
    use crate::ports::http::Response;

    #[test]
    fn a_short_error_body_is_carried_whole() {
        let response = Response {
            status: 500,
            body: "database is locked".to_owned(),
        };
        assert_eq!(describe(&response), "HTTP 500: database is locked");
    }

    #[test]
    fn an_empty_error_body_is_just_the_status() {
        let response = Response {
            status: 503,
            body: "   ".to_owned(),
        };
        assert_eq!(describe(&response), "HTTP 503");
    }

    #[test]
    fn a_long_error_body_is_clipped_and_marked() {
        // A verbose error page (which could echo a submitted field) is carried only
        // to a diagnostic length, marked as cut, not folded whole into the refusal.
        let response = Response {
            status: 500,
            body: "x".repeat(500),
        };
        let detail = describe(&response);
        assert!(
            detail.contains('…'),
            "clipped detail is marked as cut: {detail}"
        );
        assert!(
            detail.chars().count() < 260,
            "the 500-char body is not carried whole"
        );
    }
}
