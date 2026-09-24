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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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

/// A pasted credential with the whitespace around it removed.
///
/// A key copied from a provider's dashboard arrives with a trailing newline more often
/// than not, and it authenticates nowhere. Refusing it teaches the operator nothing they
/// can act on, so it is read as the key they meant.
///
/// Whitespace inside the value is left alone. That is not a paste artefact, and removing
/// it would quietly change a value that legitimately contains one.
#[must_use]
pub fn pasted(value: &str) -> String {
    value.trim().to_owned()
}

/// What proving a credential against its live service established — never the
/// input, only the outcome.
///
/// Read back as well as built. A surface that is not in this process asks setup to
/// prove a credential and is told what came of it, so the four outcomes are tagged
/// by name rather than distinguished by which field is present — the same reason an
/// answer carries the step it belongs to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
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

impl Validation {
    /// The same outcome with any credential in it withheld.
    ///
    /// What a service says when it refuses a credential is said with the credential in
    /// hand — an indexer quotes the key back inside its own `description`, and that
    /// sentence is carried here verbatim. The outcome is serialised into
    /// [`crate::model::WizardReport`] during first-run setup, which is the moment those
    /// credentials are being entered, and printed to a terminal by the prompt beside it.
    ///
    /// Safe to apply twice: the marker is recognised where a value would be, so a
    /// detail that has already been through the rule comes back unchanged rather than
    /// half-withheld again.
    #[must_use]
    pub fn withheld(self) -> Self {
        let said = |text: String| crate::config::store::withheld_text(&text);
        match self {
            Self::Valid { observed } => Self::Valid {
                observed: said(observed),
            },
            Self::Rejected { detail } => Self::Rejected {
                detail: said(detail),
            },
            Self::Unreachable { detail } => Self::Unreachable {
                detail: said(detail),
            },
            Self::Degraded { detail } => Self::Degraded {
                detail: said(detail),
            },
        }
    }
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
    /// Whether a total loss of network has already been reported by this validator.
    ///
    /// One outage, said once. Every credential proven afterwards would report the same
    /// failure for the same reason, and a list of them reads as several bad keys.
    network_said: AtomicBool,
}

impl Live {
    /// A live validator reaching services over `http`, with no Usenet transport —
    /// for a caller that proves only what HTTP answers.
    #[must_use]
    pub fn new(http: Arc<dyn Http>) -> Self {
        Self {
            http,
            nntp: None,
            network_said: AtomicBool::new(false),
        }
    }

    /// The same, also able to prove a Usenet provider over `nntp`.
    #[must_use]
    pub(crate) fn with_nntp(http: Arc<dyn Http>, nntp: Arc<dyn Nntp>) -> Self {
        Self {
            http,
            nntp: Some(nntp),
            network_said: AtomicBool::new(false),
        }
    }

    /// What a transport failure amounts to: how long it was waited for, and whether
    /// the network itself is down — stated once rather than against each credential.
    fn not_reached(&self, said: &str, reason: &str, elapsed: Duration) -> Validation {
        let network = reading::network_itself(reason);
        // Said once for one outage, not once for the lifetime of the process. A
        // validator on a served surface outlives any number of them, and a failure
        // that reached something is proof the last one ended — so the next is new,
        // and is explained in full rather than waved at an outage long since over.
        let already = if network {
            self.network_said.swap(true, Ordering::Relaxed)
        } else {
            self.network_said.store(false, Ordering::Relaxed);
            false
        };
        reading::not_reached(said, elapsed, network, already)
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
        let started = Instant::now();
        match nntp.converse(&endpoint, &commands).await {
            Ok(replies) => interpret_usenet(&replies),
            Err(unreachable) => {
                self.not_reached(&unreachable.reason, &unreachable.reason, started.elapsed())
            }
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

        let started = Instant::now();
        let response = match self.http.send(&request).await {
            Ok(response) => response,
            Err(unreachable) => {
                let said = persisting(&unreachable);
                return self.not_reached(&said, &unreachable.reason, started.elapsed());
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

        let started = Instant::now();
        match self.http.send(&request).await {
            Ok(response) => interpret_service(response.status, &response.body),
            Err(unreachable) => {
                let said = persisting(&unreachable);
                self.not_reached(&said, &unreachable.reason, started.elapsed())
            }
        }
    }
}

mod allowed;
mod reading;

pub use allowed::Allowed;
use reading::{interpret_indexer, interpret_service, interpret_usenet, persisting};

#[async_trait]
impl Validator for Live {
    async fn validate(&self, credential: &Credential) -> Validation {
        validated(self, credential).await
    }
}

async fn validated(live: &Live, credential: &Credential) -> Validation {
    let came_to = match credential {
        Credential::Indexer { url, key } => live.indexer(url, key).await,
        Credential::Service { url, key } => live.service(url, key).await,
        Credential::Usenet {
            host,
            port,
            secure,
            user,
            pass,
        } => live.usenet(host, *port, *secure, user, pass).await,
    };
    // Withheld as the service's words cross into the model, which is the one point
    // every one of them passes. What reads an outcome afterwards — a report that is
    // serialised to a caller, a prompt that prints it to a terminal — is reading text
    // the rule has already been applied to rather than remembering to apply it.
    // Anything that was answered — proven, refused, or answered and unusable — is
    // proof the network is back, so an outage after it is a new one to explain in
    // full rather than the one already reported.
    if !matches!(came_to, Validation::Unreachable { .. }) {
        live.network_said.store(false, Ordering::Relaxed);
    }
    came_to.withheld()
}

#[cfg(test)]
mod tests;
