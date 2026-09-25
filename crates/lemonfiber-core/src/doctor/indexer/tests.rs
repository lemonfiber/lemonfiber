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
