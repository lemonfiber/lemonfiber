//! Re-proving the indexer credential, so a key that rots is caught in time.
//!
//! Setup proves the indexer key the moment it is entered, but keys rot: an
//! account lapses, a key is rotated, a plan changes. So the same proof is also an
//! ordinary diagnostic check — re-runnable at any time, and part of the stack's
//! ongoing health rather than a one-off gate at setup. It asks the very
//! [`Validator`] setup used, so what "working" means cannot drift between the two.
//!
//! The credential itself never leaves this module — not into a finding, a log, or
//! a bundle. What is reported is the outcome, never the input.

use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::config::Indexer;
use crate::error::{Code, Problem, Remedy, Severity, State};
use crate::validate::{Credential, Validation, Validator};

/// Raised when the indexer answers and refuses the key it was given.
pub const INDEXER_REJECTED: Code = Code::new("CRED-2");

/// Raised when the indexer authenticates the key but cannot serve it right now.
pub const INDEXER_LIMITED: Code = Code::new("CRED-3");

/// Re-proves the configured indexer against its live service.
pub struct IndexerCheck {
    validator: Arc<dyn Validator>,
    indexer: Option<Indexer>,
}

impl IndexerCheck {
    /// A check that proves `indexer` through `validator`, or reports there is none
    /// to prove where the operator configured no indexer.
    #[must_use]
    pub fn new(validator: Arc<dyn Validator>, indexer: Option<Indexer>) -> Self {
        Self { validator, indexer }
    }

    /// Prove the indexer and read what came of it into a verdict, told apart by
    /// cause the same way setup tells them apart.
    async fn prove(&self, indexer: &Indexer) -> Verdict {
        let credential = Credential::Indexer {
            url: indexer.url.clone(),
            key: indexer.key.clone(),
        };
        match self.validator.validate(&credential).await {
            Validation::Valid { observed } => Verdict::Pass {
                note: Some(observed),
            },
            Validation::Rejected { detail } => Verdict::Fail(rejected(&detail)),
            Validation::Degraded { detail } => Verdict::Warn(limited(&detail)),
            Validation::Unreachable { detail } => Verdict::Unverified {
                reason: format!(
                    "the indexer did not answer, so its key could not be proven: {detail}"
                ),
                remedy: Remedy::new("Check the indexer is reachable, then check again"),
            },
        }
    }
}

#[async_trait]
impl Check for IndexerCheck {
    fn category(&self) -> Category {
        Category::Credentials
    }

    async fn run(&self) -> Vec<Finding> {
        let verdict = match &self.indexer {
            None => Verdict::Skipped {
                reason: "no indexer is configured, so there is none to prove".to_owned(),
            },
            Some(indexer) => self.prove(indexer).await,
        };
        vec![Finding::in_category(
            Category::Credentials,
            "credentials.indexer",
            "Indexer credential",
            verdict,
        )]
    }
}

/// The indexer answered and refused the key: the key is wrong for it, which no
/// restart fixes — it has to be corrected.
fn rejected(detail: &str) -> Problem {
    Problem::new(
        INDEXER_REJECTED,
        Severity::Error,
        "The indexer refused its key",
        "The indexer answered and rejected the API key configured for it. The key is wrong, expired, or for a different indexer — searches through it will simply come back empty.",
        Remedy::new("Correct the indexer's API key in configuration, then check again"),
    )
    .in_state(State::Guided)
    .with_detail(detail.to_owned())
}

/// The indexer authenticated the key but is limiting it — a transient state, not
/// a wrong key, so it is a warning to retry rather than an error to fix.
fn limited(detail: &str) -> Problem {
    Problem::new(
        INDEXER_LIMITED,
        Severity::Warning,
        "The indexer is limiting its key",
        "The indexer accepted the key but would not serve the request, usually a rate or quota limit that lifts on its own. The key is not wrong.",
        Remedy::new("Leave it a while and check again"),
    )
    .in_state(State::Guided)
    .with_detail(detail.to_owned())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::{IndexerCheck, INDEXER_LIMITED, INDEXER_REJECTED};
    use crate::config::Indexer;
    use crate::doctor::{Category, Check, Verdict};
    use crate::validate::{Credential, Validation, Validator};

    /// A validator that answers with a fixed outcome, so the check is driven with
    /// no network.
    struct Fixed(Validation);

    #[async_trait]
    impl Validator for Fixed {
        async fn validate(&self, _credential: &Credential) -> Validation {
            self.0.clone()
        }
    }

    /// The check over a validator that will answer `outcome`, with an indexer
    /// configured to prove.
    fn checking(outcome: Validation) -> IndexerCheck {
        IndexerCheck::new(
            Arc::new(Fixed(outcome)),
            Some(Indexer {
                url: "http://indexer.test/api".to_owned(),
                key: "the-key".to_owned(),
            }),
        )
    }

    #[tokio::test]
    async fn it_reports_in_the_credentials_category() {
        let check = checking(Validation::Valid {
            observed: String::new(),
        });
        assert_eq!(check.category(), Category::Credentials);
    }

    #[tokio::test]
    async fn a_proven_indexer_passes_with_the_observed_capability() {
        let findings = checking(Validation::Valid {
            observed: "answered a search — 9 result(s) offered".to_owned(),
        })
        .run()
        .await;
        assert!(matches!(
            findings.first().map(|finding| &finding.verdict),
            Some(Verdict::Pass { note: Some(note) }) if note.contains("9 result")
        ));
    }

    #[tokio::test]
    async fn a_refused_key_fails_and_says_to_correct_it() {
        let findings = checking(Validation::Rejected {
            detail: "incorrect credentials".to_owned(),
        })
        .run()
        .await;
        assert!(matches!(
            findings.first().map(|finding| &finding.verdict),
            Some(Verdict::Fail(problem)) if problem.code == INDEXER_REJECTED
        ));
    }

    #[tokio::test]
    async fn a_limited_key_warns_rather_than_fails() {
        let findings = checking(Validation::Degraded {
            detail: "rate-limited".to_owned(),
        })
        .run()
        .await;
        assert!(matches!(
            findings.first().map(|finding| &finding.verdict),
            Some(Verdict::Warn(problem)) if problem.code == INDEXER_LIMITED
        ));
    }

    #[tokio::test]
    async fn an_unreachable_indexer_is_unproven_not_broken() {
        let findings = checking(Validation::Unreachable {
            detail: "connection refused".to_owned(),
        })
        .run()
        .await;
        assert!(matches!(
            findings.first().map(|finding| &finding.verdict),
            Some(Verdict::Unverified { .. })
        ));
    }

    #[tokio::test]
    async fn no_configured_indexer_is_skipped_not_faulted() {
        let check = IndexerCheck::new(
            Arc::new(Fixed(Validation::Valid {
                observed: String::new(),
            })),
            None,
        );
        let findings = check.run().await;
        assert!(matches!(
            findings.first().map(|finding| &finding.verdict),
            Some(Verdict::Skipped { reason }) if reason.contains("no indexer")
        ));
    }
}
