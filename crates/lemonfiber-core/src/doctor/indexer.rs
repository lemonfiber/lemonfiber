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
//!
//! This one is not gated behind the operator asking for it, and the check beside it
//! that searches for releases is. The difference is not which of them touches the
//! network. Nothing here is taken away from the running stack: the search is one
//! request, sent to be answered, and it costs a single hit against the cap the indexer
//! holds the operator to — a share small enough that the check cannot itself be what
//! exhausts the cap. Reading the key out of configuration instead would establish only
//! that a key is written down, which is the inference this whole subsystem exists to
//! refuse, and a key that rotted overnight would go on reading as healthy until
//! something failed to download. The releases check spends one search per service every
//! time it runs and searches for content rather than for an answer, which is a large
//! enough share of the same cap to be worth asking about first.

use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::config::Indexer;
use crate::error::{Problem, Remedy, Severity, State};
use crate::validate::{Credential, Validation, Validator};

pub(crate) use crate::error::codes::cred::INDEXER_REJECTED;

pub(crate) use crate::error::codes::cred::INDEXER_LIMITED;

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
        ran(self).await
    }
}

/// Whether the indexer's credential is there and answers.
async fn ran(check: &IndexerCheck) -> Vec<Finding> {
    let verdict = match &check.indexer {
        None => Verdict::Skipped {
            reason: "no indexer is configured, so there is none to prove".to_owned(),
        },
        Some(indexer) => check.prove(indexer).await,
    };
    vec![Finding::in_category(
        Category::Credentials,
        "credentials.indexer",
        "Indexer credential",
        verdict,
    )]
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
mod tests;
