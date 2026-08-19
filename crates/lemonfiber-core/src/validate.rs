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

use crate::endpoint::API_KEY_HEADER;
use crate::ports::http::{Http, Method, Request};
use crate::ports::nntp::{Endpoint, Nntp};

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
    /// A Usenet provider, reached over NNTP and asked to accept a login.
    Usenet {
        /// The provider's hostname.
        host: String,
        /// The port it answers NNTP on.
        port: u16,
        /// Whether the connection is TLS-wrapped.
        secure: bool,
        /// The account username.
        user: String,
        /// The account password.
        pass: String,
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
/// each one speaks — the HTTP the indexers and services answer on, and, where a
/// transport for it is supplied, the NNTP a Usenet provider does.
pub struct Live {
    http: Arc<dyn Http>,
    /// How Usenet providers are reached, where the caller can reach them.
    ///
    /// Optional because not every caller proves Usenet: a diagnosis re-proving a
    /// stored indexer has no provider to speak to and supplies none, while setup,
    /// which may, supplies one. A `Usenet` credential with no transport is reported
    /// unreachable rather than pretended proven.
    nntp: Option<Arc<dyn Nntp>>,
}

impl Live {
    /// A live validator reaching services over `http`, with no Usenet transport —
    /// for a caller that proves only what HTTP answers.
    #[must_use]
    pub fn new(http: Arc<dyn Http>) -> Self {
        Self { http, nntp: None }
    }

    /// The same, also able to prove a Usenet provider over `nntp`.
    #[must_use]
    pub fn with_nntp(http: Arc<dyn Http>, nntp: Arc<dyn Nntp>) -> Self {
        Self {
            http,
            nntp: Some(nntp),
        }
    }

    /// Prove a Usenet provider by opening a connection and asking it to accept a
    /// login — the reply codes say whether it did. Where no NNTP transport was
    /// supplied there is nothing to ask, so the credential is left unproven.
    async fn usenet(
        &self,
        host: &str,
        port: u16,
        secure: bool,
        user: &str,
        pass: &str,
    ) -> Validation {
        let Some(nntp) = &self.nntp else {
            return Validation::Unreachable {
                detail: "no Usenet transport is configured to reach the provider".to_owned(),
            };
        };
        // A password must never cross a plaintext connection. On a non-TLS endpoint
        // the AUTHINFO PASS below would put the provider's account password on the
        // wire in the clear, where anyone on the path could read it, so the
        // credential is left unproven rather than exposed to prove it — the fix is
        // the operator's, to reach the provider over TLS (usually port 563).
        if !secure {
            return Validation::Unreachable {
                detail: "the provider must be reached over TLS to prove a password; a plaintext connection would send it in the clear, so it was not sent — use the provider's TLS port".to_owned(),
            };
        }
        let endpoint = Endpoint {
            host: host.to_owned(),
            port,
            secure,
        };
        // AUTHINFO is a fixed two-step: the username, then the password. The
        // provider's reply to the password is what says whether the login took.
        let commands = vec![
            format!("AUTHINFO USER {user}"),
            format!("AUTHINFO PASS {pass}"),
        ];
        match nntp.converse(&endpoint, &commands).await {
            Ok(replies) => interpret_usenet(&replies),
            Err(unreachable) => Validation::Unreachable {
                detail: unreachable.reason,
            },
        }
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
                    detail: persisting(&unreachable),
                }
            }
        };

        // Torznab authenticates by a key in the query string, so a key proven over
        // plaintext http rode the wire — and sits in the indexer's access logs — in
        // the clear. It is still proven, but the exposure is named so the operator
        // can move to https, where the same key would at least be encrypted in transit.
        match interpret_indexer(response.status, &response.body) {
            Validation::Valid { observed } if url.to_ascii_lowercase().starts_with("http://") => {
                Validation::Valid {
                    observed: format!(
                        "{observed} — but the indexer was reached over plaintext http, so its key was exposed on the wire; use an https URL"
                    ),
                }
            }
            other => other,
        }
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
                detail: persisting(&unreachable),
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
            Credential::Usenet {
                host,
                port,
                secure,
                user,
                pass,
            } => self.usenet(host, *port, *secure, user, pass).await,
        }
    }
}

/// Read a service's answer to an authenticated identity request into an outcome.
///
/// What a transport failure amounts to, in the operator's terms.
///
/// The transport's own words lead, as they must — they are the only account of
/// what actually happened. What lemonfiber adds is whether it kept happening: a
/// service that did not answer once may have been busy, and one that did not
/// answer every time it was asked is down, and those are different things to do
/// about.
fn persisting(unreachable: &crate::ports::http::Unreachable) -> String {
    match crate::retry::said(unreachable.attempts) {
        Some(persisted) => format!("{} — {persisted}", unreachable.reason),
        None => unreachable.reason.clone(),
    }
}

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

/// Read a provider's replies to a login into an outcome.
///
/// The reply to the password is what decides it: accepted proves the login,
/// refused is a wrong username or password, and a permission or connection-limit
/// code is an account that authenticated but cannot serve right now. A greeting
/// that already turned the connection away — the provider at its limit — is that
/// same degraded case, told before the login was even reached. Too few replies to
/// have finished the exchange leave it unreachable.
fn interpret_usenet(replies: &[String]) -> Validation {
    // A provider that turns the connection away at the greeting never reaches the
    // login; its limit is the reason, not the credential.
    if matches!(replies.first().and_then(|line| code(line)), Some(400 | 502)) {
        return Validation::Degraded {
            detail: "the provider is at its connection limit; try again shortly".to_owned(),
        };
    }
    // Some providers accept at the username step and ask for no password; the login
    // is already proven, and the password reply that follows would read as an
    // out-of-sequence refusal if it were taken as the answer.
    if matches!(
        replies.get(1).and_then(|line| code(line)),
        Some(AUTH_ACCEPTED)
    ) {
        return Validation::Valid {
            observed: "the provider accepted the login".to_owned(),
        };
    }
    // The greeting, the reply to the username, then the reply to the password.
    let Some(password_reply) = replies.get(2) else {
        return Validation::Unreachable {
            detail: "the provider did not complete the login exchange".to_owned(),
        };
    };
    match code(password_reply) {
        Some(AUTH_ACCEPTED) => Validation::Valid {
            observed: "the provider accepted the login".to_owned(),
        },
        Some(AUTH_REJECTED | AUTH_OUT_OF_SEQUENCE) => Validation::Rejected {
            detail: "the provider refused the username or password".to_owned(),
        },
        Some(NO_PERMISSION) => Validation::Degraded {
            detail: "the provider accepted the account but would not serve it — usually its \
                     connection limit"
                .to_owned(),
        },
        Some(other) => Validation::Rejected {
            detail: format!("the provider answered {other} to the login"),
        },
        None => Validation::Unreachable {
            detail: "the provider did not answer the login in a way that could be read".to_owned(),
        },
    }
}

/// The NNTP reply code for an accepted authentication.
const AUTH_ACCEPTED: u16 = 281;

/// The code for a rejected authentication.
const AUTH_REJECTED: u16 = 481;

/// The code for an authentication command out of sequence — the login as a whole
/// did not take, so it reads the same as a refusal to the operator.
const AUTH_OUT_OF_SEQUENCE: u16 = 482;

/// The code for a command the account is not permitted — on a login, an account
/// that is known but cannot be served, typically at its connection limit.
const NO_PERMISSION: u16 = 502;

/// The three-digit status code an NNTP reply line begins with, where it begins
/// with one — the only part of the line an outcome turns on.
fn code(line: &str) -> Option<u16> {
    let digits = line.get(0..3)?;
    if digits.bytes().all(|byte| byte.is_ascii_digit()) {
        digits.parse().ok()
    } else {
        None
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

    use lemonfiber_fixtures::http::{Answer, Fake};

    use super::{Credential, Live, Validation, Validator};
    use crate::ports::http::{Http, Request, Response, Unreachable};
    use crate::ports::nntp::{Endpoint, Nntp};

    /// A validator whose transport answers with the given body at 200.
    fn answering(body: &str) -> Live {
        Live::new(Fake::always(Answer::reply(200, body.to_owned())))
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
    async fn a_key_proven_over_plaintext_http_is_named_as_exposed() {
        // The default indexer URL is http://, so a proven key carries the caveat
        // that it rode the wire in the clear, pointing the operator at https.
        let feed = "<?xml version=\"1.0\"?><rss><channel><item>a</item></channel></rss>";
        let outcome = answering(feed).validate(&indexer()).await;
        assert!(matches!(
            outcome,
            Validation::Valid { observed }
                if observed.contains("plaintext http") && observed.contains("https")
        ));
    }

    #[tokio::test]
    async fn a_key_proven_over_https_carries_no_plaintext_caveat() {
        let feed = "<?xml version=\"1.0\"?><rss><channel><item>a</item></channel></rss>";
        let secure = Credential::Indexer {
            url: "https://indexer.test/api".to_owned(),
            key: "abc".to_owned(),
        };
        let outcome = answering(feed).validate(&secure).await;
        assert!(matches!(
            outcome,
            Validation::Valid { observed } if !observed.contains("plaintext")
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

    #[test]
    fn a_service_that_kept_not_answering_is_told_apart_from_one_that_was_busy() {
        // The transport's own words lead either way — they are the only account of
        // what happened — and what lemonfiber adds is whether it kept happening.
        let once = crate::ports::http::Unreachable::once("http://sonarr", "connection refused");
        assert_eq!(super::persisting(&once), "connection refused");

        let persisted = crate::ports::http::Unreachable {
            attempts: crate::retry::ATTEMPTS,
            ..once
        };
        assert_eq!(
            super::persisting(&persisted),
            "connection refused — still failing after 3 attempts"
        );
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
        let outcome = Live::new(Fake::always(Answer::reply(401, String::new())))
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
        let outcome = Live::new(Fake::silent()).validate(&indexer()).await;
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

    /// An NNTP transport that answers `converse` with scripted reply lines, or
    /// with nothing at all where the connection could not be made.
    struct Dialogue(Result<Vec<String>, crate::ports::nntp::Unreachable>);

    #[async_trait]
    impl Nntp for Dialogue {
        async fn converse(
            &self,
            _endpoint: &Endpoint,
            _commands: &[String],
        ) -> Result<Vec<String>, crate::ports::nntp::Unreachable> {
            self.0.clone()
        }
    }

    /// A validator that reaches Usenet through a transport scripted to reply with
    /// `lines` (greeting, then a reply per command).
    fn dialling(lines: &[&str]) -> Live {
        let replies = lines.iter().map(|line| (*line).to_owned()).collect();
        Live::with_nntp(
            Fake::always(Answer::reply(200, String::new())),
            Arc::new(Dialogue(Ok(replies))),
        )
    }

    /// The Usenet credential the tests prove; host and port are immaterial to a
    /// scripted transport.
    fn usenet() -> Credential {
        Credential::Usenet {
            host: "news.provider.test".to_owned(),
            port: 563,
            secure: true,
            user: "person".to_owned(),
            pass: "secret".to_owned(),
        }
    }

    #[tokio::test]
    async fn a_provider_that_accepts_the_login_proves_the_account() {
        // Greeting, reply to USER, then 281 accepted to PASS.
        let outcome = dialling(&["200 welcome", "381 more", "281 authenticated"])
            .validate(&usenet())
            .await;
        assert!(matches!(
            outcome,
            Validation::Valid { observed } if observed.contains("accepted the login")
        ));
    }

    #[tokio::test]
    async fn a_provider_that_accepts_at_the_username_step_needs_no_password() {
        // 281 to AUTHINFO USER means the login is already accepted; the password
        // reply that still follows must not be mistaken for a refusal.
        let outcome = dialling(&["200 welcome", "281 authenticated", "482 out of sequence"])
            .validate(&usenet())
            .await;
        assert!(matches!(
            outcome,
            Validation::Valid { observed } if observed.contains("accepted the login")
        ));
    }

    #[tokio::test]
    async fn a_provider_that_refuses_the_login_is_rejected() {
        let outcome = dialling(&["200 welcome", "381 more", "481 authentication failed"])
            .validate(&usenet())
            .await;
        assert!(matches!(
            outcome,
            Validation::Rejected { detail } if detail.contains("username or password")
        ));
    }

    #[tokio::test]
    async fn a_provider_at_its_connection_limit_at_the_greeting_is_degraded() {
        // 502 at the greeting turns the connection away before the login.
        let outcome = dialling(&["502 too many connections"])
            .validate(&usenet())
            .await;
        assert!(matches!(
            outcome,
            Validation::Degraded { detail } if detail.contains("connection limit")
        ));
    }

    #[tokio::test]
    async fn a_login_refused_for_no_permission_is_degraded_not_a_wrong_password() {
        let outcome = dialling(&["200 welcome", "381 more", "502 no permission"])
            .validate(&usenet())
            .await;
        assert!(matches!(
            outcome,
            Validation::Degraded { detail } if detail.contains("connection limit")
        ));
    }

    #[tokio::test]
    async fn an_out_of_sequence_login_reads_as_a_refusal() {
        let outcome = dialling(&["200 welcome", "381 more", "482 out of sequence"])
            .validate(&usenet())
            .await;
        assert!(matches!(outcome, Validation::Rejected { .. }));
    }

    #[tokio::test]
    async fn an_unexpected_login_code_is_refused_carrying_the_number() {
        let outcome = dialling(&["200 welcome", "381 more", "400 service discontinued"])
            .validate(&usenet())
            .await;
        assert!(matches!(
            outcome,
            Validation::Rejected { detail } if detail.contains("400")
        ));
    }

    #[tokio::test]
    async fn a_login_reply_without_a_code_cannot_be_read() {
        let outcome = dialling(&["200 welcome", "381 more", "not a coded line"])
            .validate(&usenet())
            .await;
        assert!(matches!(outcome, Validation::Unreachable { .. }));
    }

    #[tokio::test]
    async fn an_exchange_that_did_not_finish_is_unreachable() {
        // Only a greeting came back — the login never completed.
        let outcome = dialling(&["200 welcome"]).validate(&usenet()).await;
        assert!(matches!(
            outcome,
            Validation::Unreachable { detail } if detail.contains("did not complete")
        ));
    }

    #[tokio::test]
    async fn a_provider_that_cannot_be_reached_is_unreachable() {
        let live = Live::with_nntp(
            Fake::always(Answer::reply(200, String::new())),
            Arc::new(Dialogue(Err(crate::ports::nntp::Unreachable {
                host: "news.provider.test".to_owned(),
                reason: "connection refused".to_owned(),
            }))),
        );
        assert!(matches!(
            live.validate(&usenet()).await,
            Validation::Unreachable { detail } if detail.contains("connection refused")
        ));
    }

    #[tokio::test]
    async fn a_usenet_credential_with_no_transport_is_unproven_not_pretended() {
        // The HTTP-only validator has no NNTP transport, so a Usenet credential
        // cannot be proven — reported unreachable rather than passed.
        let outcome = answering(String::new().as_str()).validate(&usenet()).await;
        assert!(matches!(
            outcome,
            Validation::Unreachable { detail } if detail.contains("no Usenet transport")
        ));
    }

    #[tokio::test]
    async fn a_plaintext_provider_is_left_unproven_rather_than_exposing_the_password() {
        // The transport is scripted to accept the login, so were the password sent
        // the outcome would be Valid. Over a non-TLS endpoint it is refused before
        // anything is sent — unreachable, naming TLS as the fix — so the password
        // never reaches the wire.
        let insecure = Credential::Usenet {
            host: "news.provider.test".to_owned(),
            port: 119,
            secure: false,
            user: "person".to_owned(),
            pass: "secret".to_owned(),
        };
        let outcome = dialling(&["200 welcome", "381 more", "281 authenticated"])
            .validate(&insecure)
            .await;
        assert!(matches!(
            outcome,
            Validation::Unreachable { detail }
                if detail.contains("TLS") && detail.contains("in the clear")
        ));
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
        let outcome = Live::new(Fake::always(Answer::reply(401, String::new())))
            .validate(&service())
            .await;
        assert!(matches!(
            outcome,
            Validation::Rejected { detail } if detail.contains("401")
        ));
    }

    #[tokio::test]
    async fn a_service_url_that_answers_as_something_else_points_at_the_url() {
        let outcome = Live::new(Fake::always(Answer::reply(
            404,
            "<html>not found</html>".to_owned(),
        )))
        .validate(&service())
        .await;
        assert!(matches!(
            outcome,
            Validation::Unreachable { detail } if detail.contains("check the URL")
        ));
    }

    #[tokio::test]
    async fn a_service_that_does_not_answer_is_unreachable() {
        let outcome = Live::new(Fake::silent()).validate(&service()).await;
        assert!(matches!(
            outcome,
            Validation::Unreachable { detail } if detail.contains("connection refused")
        ));
    }

    #[tokio::test]
    async fn a_service_is_reached_with_its_key_in_the_api_header() {
        let recording = Fake::always(Answer::reply(200, r#"{"instanceName":"Radarr"}"#));
        let outcome = Live::new(recording.clone()).validate(&service()).await;

        assert!(matches!(outcome, Validation::Valid { .. }));
        let asked = recording.request();
        assert!(
            asked
                .as_ref()
                .is_some_and(|request| request.url.contains("system/status")),
            "the identity endpoint is reached"
        );
        assert!(
            asked.is_some_and(|request| request
                .headers
                .iter()
                .any(|(name, value)| name == "X-Api-Key" && value == "abc")),
            "the key authenticates the request as a header, not a query param"
        );
    }

    #[tokio::test]
    async fn the_search_carries_the_key_and_keeps_an_existing_query_intact() {
        let recording = Fake::always(Answer::reply(200, "<rss><channel></channel></rss>"));
        let credential = Credential::Indexer {
            url: "http://indexer.test/api?limit=1".to_owned(),
            key: "secret".to_owned(),
        };
        let outcome = Live::new(recording.clone()).validate(&credential).await;

        assert!(matches!(outcome, Validation::Valid { .. }));
        let url = recording.request().map(|request| request.url);
        assert!(
            url.as_deref().is_some_and(|url| url.contains("t=search")),
            "a real search is issued"
        );
        assert!(
            url.as_deref()
                .is_some_and(|url| url.contains("apikey=secret")),
            "the key authenticates the query"
        );
        assert!(
            url.as_deref().is_some_and(|url| url.contains("?limit=1&")),
            "an existing query is joined with & not a second ?"
        );
    }
}
