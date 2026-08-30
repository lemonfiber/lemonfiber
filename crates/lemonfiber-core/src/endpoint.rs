//! The plumbing every service client shares over the HTTP port.
//!
//! Servarr and qBittorrent speak different APIs, but they reach them the same
//! way: a base address, a name to blame a failure on, and one reading of what a
//! transport error or a non-success status means. That common part lives here so
//! it is written once; each client builds its own requests and holds one of these
//! to send them.

use std::sync::Arc;

use crate::ports::http::{Http, Method, Request, Response};
use crate::ports::service::Failure;
use crate::text::fitted;

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

/// The media type a form request declares — the shape services older than JSON APIs
/// read their settings in, and the only one they parse.
const FORM: &str = "application/x-www-form-urlencoded";

/// The `Content-Type` header a form request carries.
///
/// Not an `Option`, unlike JSON's: a form request is by definition one with a body,
/// so there is no bodiless case to fold away.
pub(crate) fn form_content_type() -> (String, String) {
    ("Content-Type".to_owned(), FORM.to_owned())
}

/// Render form fields as an `application/x-www-form-urlencoded` body.
///
/// Infallible by construction, so there is no encoding error to fold into the
/// result: every field is a string, and every string has an encoding.
pub(crate) fn form_encoded(fields: &[(&str, &str)]) -> String {
    let mut form = form_urlencoded::Serializer::new(String::new());
    for (name, value) in fields {
        form.append_pair(name, value);
    }
    form.finish()
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

    /// A JSON request to `path` — the body declared as JSON where there is one, and no
    /// API key. The shape the services that authenticate some other way build from: a
    /// media server driven through its own body-carried credential, a request manager
    /// signed in through the media server, or a setup endpoint that takes no key yet.
    pub(crate) fn json_request(&self, method: Method, path: &str, body: Option<String>) -> Request {
        Request {
            method,
            url: self.url(path),
            headers: json_content_type(body.as_ref()).into_iter().collect(),
            body,
        }
    }

    /// A JSON request that also carries a Servarr-shape API key in its header — the shape
    /// every keyed service builds from (Sonarr, Radarr, Lidarr, Prowlarr), differing only
    /// in the path prefix the caller supplies for its API version.
    pub(crate) fn keyed_request(
        &self,
        method: Method,
        path: &str,
        key: &str,
        body: Option<String>,
    ) -> Request {
        let mut headers = vec![(API_KEY_HEADER.to_owned(), key.to_owned())];
        headers.extend(json_content_type(body.as_ref()));
        Request {
            method,
            url: self.url(path),
            headers,
            body,
        }
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
/// where it gave any — shortened to a diagnostic length — its status code alone
/// otherwise.
///
/// Shortened by [`fitted`], which elides the middle and keeps both ends. A
/// service's error text commonly opens with boilerplate and closes with the
/// specific failure, so the end is the half that names the cause, and a body cut
/// at the tail keeps the half every such body shares.
pub(crate) fn describe(response: &Response) -> String {
    let body = response.body.trim();
    if body.is_empty() {
        format!("HTTP {}", response.status)
    } else {
        format!("HTTP {}: {}", response.status, fitted(body, DETAIL_LIMIT))
    }
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
    fn a_long_error_body_is_shortened_and_marked() {
        // A verbose error page (which could echo a submitted field) is carried only
        // to a diagnostic length, marked where it was shortened, not folded whole
        // into the refusal.
        let response = Response {
            status: 500,
            body: "x".repeat(500),
        };
        let detail = describe(&response);
        assert!(
            detail.contains("..."),
            "a shortened detail is marked as shortened: {detail}"
        );
        assert!(
            detail.chars().count() < 260,
            "the 500-char body is not carried whole"
        );
    }

    /// The defect this exists for. A service's error text opens with boilerplate
    /// and closes with the failure it is reporting; a body cut at the tail keeps
    /// the half every such body shares and drops the half that names the cause.
    #[test]
    fn a_long_error_body_still_names_the_cause_at_its_end() {
        let response = Response {
            status: 502,
            body: format!(
                "Bad Gateway. {}Connection refused by 10.0.0.5:8080.",
                "The upstream server did not answer. ".repeat(20)
            ),
        };

        let detail = describe(&response);

        assert!(detail.starts_with("HTTP 502: Bad Gateway."), "{detail}");
        assert!(
            detail.ends_with("Connection refused by 10.0.0.5:8080."),
            "{detail}"
        );
    }

    /// The marker is full stops rather than an ellipsis, so a terminal that cannot
    /// render the character is never handed one.
    #[test]
    fn shortening_an_error_body_uses_no_character_a_terminal_might_not_have() {
        let response = Response {
            status: 500,
            body: "y".repeat(500),
        };

        let detail = describe(&response);

        assert!(detail.is_ascii(), "{detail}");
    }
}
