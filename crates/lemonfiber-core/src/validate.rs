//! Proving a credential works by asking its live service, before it is stored.
//!
//! A syntactically perfect API key the indexer rejects is worthless, and a format
//! check gives false confidence — so a credential is *tested*, not inspected: the
//! service is asked to act on it, and what it answers decides the outcome. The
//! failure a wrong credential causes belongs where it was entered, not three
//! screens later as an empty search result.
//!
//! What comes back is never the input — only the outcome, and on success the
//! capability observed while proving it ("answered a search — 40 results"), which
//! tells the operator more than a bare "OK": it is how they notice a plan that
//! reports fewer connections than they bought, or an indexer that answers but
//! finds nothing.
//!
//! The three ways a test can fall short are kept apart, because their remedies
//! are: a service that **answered and refused** (check the key), one that **did
//! not answer** (check the host, or your own connectivity), and one that
//! **authenticated but cannot do its job** (the account is exhausted or limited).
//! Collapsing them into "validation failed" sends the operator after the wrong
//! problem.

use std::sync::Arc;

use async_trait::async_trait;

use crate::ports::http::{Http, Method, Request};

/// A credential the operator supplies, to be proven against the live service it
/// authenticates to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credential {
    /// A Torznab or Newznab indexer: the API base URL, and the key it takes.
    Indexer {
        /// The indexer's API base URL, as the operator gave it.
        url: String,
        /// The API key the indexer authenticates the query with.
        key: String,
    },
    /// An existing service to adopt — a Servarr-shape API reached with its key,
    /// asked to read back its own identity.
    Service {
        /// The service's base URL.
        url: String,
        /// The API key it authenticates with, sent as `X-Api-Key`.
        key: String,
    },
}

/// What proving a credential against its live service established — never the
/// input, only the outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validation {
    /// Proven working, carrying the capability observed while proving it.
    Valid {
        /// The observed fact — what the service did, not that it merely answered.
        observed: String,
    },
    /// The service answered and refused: the credential is wrong for it.
    Rejected {
        /// What the service said, in terms the operator can act on.
        detail: String,
    },
    /// Nothing usable answered, so nothing can be concluded about the credential.
    Unreachable {
        /// Why nothing usable came back.
        detail: String,
    },
    /// It authenticated, but cannot do the job it is for — exhausted, limited, or
    /// otherwise unable.
    Degraded {
        /// What it can no longer do, and why where the service says.
        detail: String,
    },
}

/// Proves credentials against their live services.
///
/// A port, so the setup wizard drives it in a test through a scripted outcome with
/// no network, and the one implementation that speaks to real services is proven
/// on its own against a fake transport.
#[async_trait]
pub trait Validator: Send + Sync {
    /// Prove `credential` against its service, and report only what came of it.
    async fn validate(&self, credential: &Credential) -> Validation;
}

/// A validator that proves credentials against the real services, over whatever
/// each one speaks — today the HTTP the indexers answer on.
pub struct Live {
    http: Arc<dyn Http>,
}

impl Live {
    /// A live validator reaching services over `http`.
    #[must_use]
    pub fn new(http: Arc<dyn Http>) -> Self {
        Self { http }
    }

    /// Prove an indexer by issuing a real search against it and reading what came
    /// back — a well-formed result set proves the key, an error element refuses
    /// it, and nothing at all leaves it unreachable.
    async fn indexer(&self, url: &str, key: &str) -> Validation {
        // A trivial search authenticates the key and exercises the indexer, where a
        // capabilities call some indexers answer without a key would not. The
        // separator keeps a base that already carries a query intact.
        let separator = if url.contains('?') { '&' } else { '?' };
        let request = Request {
            method: Method::Get,
            url: format!("{url}{separator}t=search&apikey={key}"),
            headers: Vec::new(),
            body: None,
        };

        let response = match self.http.send(&request).await {
            Ok(response) => response,
            Err(unreachable) => {
                return Validation::Unreachable {
                    detail: unreachable.reason,
                }
            }
        };

        interpret_indexer(response.status, &response.body)
    }

    /// Prove an existing service by reaching its API with the key and reading what
    /// it says about itself — a refusal is a wrong key, an answer that is not the
    /// service's own points at the URL.
    async fn service(&self, url: &str, key: &str) -> Validation {
        let request = Request {
            method: Method::Get,
            url: url.to_owned(),
            headers: vec![(API_KEY_HEADER.to_owned(), key.to_owned())],
            body: None,
        };

        match self.http.send(&request).await {
            Ok(response) => interpret_service(response.status, &response.body),
            Err(unreachable) => Validation::Unreachable {
                detail: unreachable.reason,
            },
        }
    }
}

#[async_trait]
impl Validator for Live {
    async fn validate(&self, credential: &Credential) -> Validation {
        match credential {
            Credential::Indexer { url, key } => self.indexer(url, key).await,
            Credential::Service { url, key } => self.service(url, key).await,
        }
    }
}

/// The header a Servarr-shape service authenticates a request with.
const API_KEY_HEADER: &str = "X-Api-Key";

/// Read a service's answer to an authenticated identity request into an outcome.
///
/// A refusing status is the key being wrong; a well-formed identity proves it and
/// carries what the service said about itself as the observed capability; any
/// other answer came from something that is not the service's API, which points
/// at the URL rather than the key.
fn interpret_service(status: u16, body: &str) -> Validation {
    if status == UNAUTHORIZED || status == FORBIDDEN {
        return Validation::Rejected {
            detail: format!("the service answered {status} — the key was refused"),
        };
    }
    if (200..300).contains(&status) {
        return Validation::Valid {
            observed: identity(body),
        };
    }
    Validation::Unreachable {
        detail: format!("the service answered {status}, not as a reachable API — check the URL"),
    }
}

/// What a service says about itself, from the identity it answered with.
///
/// A Servarr status names the instance and its version; either alone is worth
/// reporting, and where neither is there the fact that the key was accepted is
/// still the observation.
fn identity(body: &str) -> String {
    let parsed = serde_json::from_str::<serde_json::Value>(body).ok();
    let field = |name: &str| {
        parsed
            .as_ref()
            .and_then(|value| value.get(name))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    match (field("instanceName"), field("version")) {
        (Some(name), Some(version)) => format!("reached {name}, version {version}"),
        (Some(name), None) => format!("reached {name}"),
        (None, Some(version)) => format!("the service accepted the key — version {version}"),
        (None, None) => "the service accepted the key".to_owned(),
    }
}

/// Read a Torznab or Newznab indexer's answer to a search into an outcome.
///
/// An error element is the indexer refusing the query, and its code says whether
/// that is the key (rejected) or a limit the account has hit (degraded, and
/// transient). A well-formed feed proves the key and carries how many results it
/// held as the observed capability. Anything else — a login page, an unrelated
/// site — answered without being the indexer, which points at the URL rather than
/// the key, so it is unreachable-for-this-purpose rather than a refusal.
fn interpret_indexer(status: u16, body: &str) -> Validation {
    if let Some(code) = error_attr(body, "code").and_then(|code| code.parse::<u32>().ok()) {
        let said = error_attr(body, "description").unwrap_or_else(|| "no reason given".to_owned());
        return match code {
            // The request-limit code is a rate-limit, not a wrong key: authenticated,
            // but temporarily unable, which is degraded and worth retrying.
            RATE_LIMITED => Validation::Degraded {
                detail: format!(
                    "the indexer is rate-limiting this key ({said}); try again shortly"
                ),
            },
            _ => Validation::Rejected {
                detail: format!("the indexer refused the key: {said}"),
            },
        };
    }

    if status == UNAUTHORIZED || status == FORBIDDEN {
        return Validation::Rejected {
            detail: format!("the indexer answered {status} — the key was refused"),
        };
    }

    let lower = body.to_ascii_lowercase();
    if lower.contains("<rss") || lower.contains("<channel") || lower.contains("<caps") {
        let results = lower.matches("<item").count();
        return Validation::Valid {
            observed: format!("answered a search — {results} result(s) offered"),
        };
    }

    Validation::Unreachable {
        detail: "answered, but not as a Torznab or Newznab indexer — check the URL".to_owned(),
    }
}

/// The Newznab error code for a request the account has rate-limited.
const RATE_LIMITED: u32 = 500;

/// The status a service returns when a credential is refused outright.
const UNAUTHORIZED: u16 = 401;

/// The status a service returns when a credential is known but not permitted.
const FORBIDDEN: u16 = 403;

/// The value of `attr="…"` (or `'…'`) on the first `<error` element in `body`, or
/// nothing where there is no such element or attribute.
///
/// A deliberately small reader rather than a full XML parse: the one element whose
/// shape matters here is the error the Newznab and Torznab specs both fix, and a
/// dependency to read one attribute off it would be its own liability.
fn error_attr(body: &str, attr: &str) -> Option<String> {
    let error = body.find("<error")?;
    let within = &body[error..];
    let at = within.find(attr)?;
    let after = within[at + attr.len()..].trim_start();
    let value = after.strip_prefix('=')?.trim_start();
    let quote = value.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &value[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_owned())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::{Credential, Live, Validation, Validator};
    use crate::ports::http::{Http, Request, Response, Unreachable};

    /// An HTTP transport that answers every request the same scripted way — either
    /// a response, or nothing at all.
    struct Canned(Result<Response, Unreachable>);

    #[async_trait]
    impl Http for Canned {
        async fn send(&self, _request: &Request) -> Result<Response, Unreachable> {
            self.0.clone()
        }
    }

    /// A validator whose transport answers with the given body at 200.
    fn answering(body: &str) -> Live {
        Live::new(Arc::new(Canned(Ok(Response {
            status: 200,
            body: body.to_owned(),
        }))))
    }

    /// The indexer credential the tests prove; the URL and key are immaterial to a
    /// scripted transport.
    fn indexer() -> Credential {
        Credential::Indexer {
            url: "http://indexer.test/api".to_owned(),
            key: "abc".to_owned(),
        }
    }

    #[tokio::test]
    async fn a_well_formed_feed_proves_the_key_and_reports_what_it_held() {
        let feed =
            "<?xml version=\"1.0\"?><rss><channel><item>a</item><item>b</item></channel></rss>";
        let outcome = answering(feed).validate(&indexer()).await;
        assert!(matches!(
            outcome,
            Validation::Valid { observed } if observed.contains("2 result")
        ));
    }

    #[tokio::test]
    async fn an_error_element_for_a_bad_key_is_a_refusal_with_the_reason() {
        let body = "<error code=\"100\" description=\"Incorrect user credentials\"/>";
        let outcome = answering(body).validate(&indexer()).await;
        assert!(matches!(
            outcome,
            Validation::Rejected { detail } if detail.contains("Incorrect user credentials")
        ));
    }

    #[tokio::test]
    async fn a_rate_limit_error_is_degraded_and_transient_not_a_refusal() {
        let body = "<error code=\"500\" description=\"Request limit reached\"/>";
        let outcome = answering(body).validate(&indexer()).await;
        assert!(matches!(
            outcome,
            Validation::Degraded { detail } if detail.contains("rate-limiting")
        ));
    }

    #[tokio::test]
    async fn an_error_without_a_description_still_refuses_rather_than_panics() {
        let body = "<error code=\"101\"/>";
        let outcome = answering(body).validate(&indexer()).await;
        assert!(matches!(
            outcome,
            Validation::Rejected { detail } if detail.contains("no reason given")
        ));
    }

    #[tokio::test]
    async fn a_refusing_status_with_no_error_element_is_still_a_refusal() {
        let outcome = Live::new(Arc::new(Canned(Ok(Response {
            status: 401,
            body: String::new(),
        }))))
        .validate(&indexer())
        .await;
        assert!(matches!(
            outcome,
            Validation::Rejected { detail } if detail.contains("401")
        ));
    }

    #[tokio::test]
    async fn a_page_that_is_not_an_indexer_points_at_the_url_not_the_key() {
        let outcome = answering("<html><body>Sign in</body></html>")
            .validate(&indexer())
            .await;
        assert!(matches!(
            outcome,
            Validation::Unreachable { detail } if detail.contains("check the URL")
        ));
    }

    #[tokio::test]
    async fn an_error_whose_code_is_not_quoted_is_not_read_as_a_refusal() {
        // A reader that only understands the fixed, quoted shape both specs use must
        // not mistake a malformed attribute for a code — it reads no code, so the
        // answer falls through to "not an indexer" rather than a false refusal.
        let outcome = answering("<error code=100/>").validate(&indexer()).await;
        assert!(matches!(outcome, Validation::Unreachable { detail } if detail.contains("URL")));
    }

    #[tokio::test]
    async fn nothing_answering_at_all_is_unreachable_with_the_transports_reason() {
        let outcome = Live::new(Arc::new(Canned(Err(Unreachable {
            url: "http://indexer.test/api".to_owned(),
            reason: "connection refused".to_owned(),
        }))))
        .validate(&indexer())
        .await;
        assert!(matches!(
            outcome,
            Validation::Unreachable { detail } if detail.contains("connection refused")
        ));
    }

    /// The service credential the tests prove.
    fn service() -> Credential {
        Credential::Service {
            url: "http://sonarr.test:8989/api/v3/system/status".to_owned(),
            key: "abc".to_owned(),
        }
    }

    #[tokio::test]
    async fn a_service_that_answers_its_identity_is_proven_and_names_itself() {
        let body = r#"{"instanceName":"Sonarr","version":"4.0.1"}"#;
        let outcome = answering(body).validate(&service()).await;
        assert!(matches!(
            outcome,
            Validation::Valid { observed } if observed.contains("Sonarr") && observed.contains("4.0.1")
        ));
    }

    #[tokio::test]
    async fn a_service_that_answers_without_naming_itself_is_still_proven() {
        // A 2xx with no recognisable identity still proves the key was accepted.
        let outcome = answering("{}").validate(&service()).await;
        assert!(matches!(
            outcome,
            Validation::Valid { observed } if observed.contains("accepted the key")
        ));
    }

    #[tokio::test]
    async fn a_service_that_gives_only_a_version_reports_that() {
        let outcome = answering(r#"{"version":"4.0.1"}"#)
            .validate(&service())
            .await;
        assert!(matches!(
            outcome,
            Validation::Valid { observed } if observed.contains("version 4.0.1")
        ));
    }

    #[tokio::test]
    async fn a_service_that_refuses_the_key_is_rejected() {
        let outcome = Live::new(Arc::new(Canned(Ok(Response {
            status: 401,
            body: String::new(),
        }))))
        .validate(&service())
        .await;
        assert!(matches!(
            outcome,
            Validation::Rejected { detail } if detail.contains("401")
        ));
    }

    #[tokio::test]
    async fn a_service_url_that_answers_as_something_else_points_at_the_url() {
        let outcome = Live::new(Arc::new(Canned(Ok(Response {
            status: 404,
            body: "<html>not found</html>".to_owned(),
        }))))
        .validate(&service())
        .await;
        assert!(matches!(
            outcome,
            Validation::Unreachable { detail } if detail.contains("check the URL")
        ));
    }

    #[tokio::test]
    async fn a_service_that_does_not_answer_is_unreachable() {
        let outcome = Live::new(Arc::new(Canned(Err(Unreachable {
            url: "http://sonarr.test:8989".to_owned(),
            reason: "connection refused".to_owned(),
        }))))
        .validate(&service())
        .await;
        assert!(matches!(
            outcome,
            Validation::Unreachable { detail } if detail.contains("connection refused")
        ));
    }

    #[tokio::test]
    async fn a_service_is_reached_with_its_key_in_the_api_header() {
        /// A request the fake saw: its URL and the headers it carried.
        type Seen = (String, Vec<(String, String)>);
        struct Recording(std::sync::Mutex<Vec<Seen>>);
        #[async_trait]
        impl Http for Recording {
            async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
                self.0
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push((request.url.clone(), request.headers.clone()));
                Ok(Response {
                    status: 200,
                    body: r#"{"instanceName":"Radarr"}"#.to_owned(),
                })
            }
        }
        let recording = Arc::new(Recording(std::sync::Mutex::new(Vec::new())));
        let outcome = Live::new(recording.clone()).validate(&service()).await;

        assert!(matches!(outcome, Validation::Valid { .. }));
        let asked = recording
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (url, headers) = asked.first().cloned().unwrap_or_default();
        assert!(
            url.contains("system/status"),
            "the identity endpoint is reached"
        );
        assert!(
            headers
                .iter()
                .any(|(name, value)| name == "X-Api-Key" && value == "abc"),
            "the key authenticates the request as a header, not a query param"
        );
    }

    #[tokio::test]
    async fn the_search_carries_the_key_and_keeps_an_existing_query_intact() {
        // A transport that records the URL it was asked for, so the request the
        // validator builds can be inspected.
        struct Recording(std::sync::Mutex<Vec<String>>);
        #[async_trait]
        impl Http for Recording {
            async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
                self.0
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(request.url.clone());
                Ok(Response {
                    status: 200,
                    body: "<rss><channel></channel></rss>".to_owned(),
                })
            }
        }
        let recording = Arc::new(Recording(std::sync::Mutex::new(Vec::new())));
        let credential = Credential::Indexer {
            url: "http://indexer.test/api?limit=1".to_owned(),
            key: "secret".to_owned(),
        };
        let outcome = Live::new(recording.clone()).validate(&credential).await;

        assert!(matches!(outcome, Validation::Valid { .. }));
        let asked = recording
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let url = asked.first().map(String::as_str).unwrap_or_default();
        assert!(url.contains("t=search"), "a real search is issued");
        assert!(
            url.contains("apikey=secret"),
            "the key authenticates the query"
        );
        assert!(
            url.contains("?limit=1&"),
            "an existing query is joined with & not a second ?"
        );
    }
}
