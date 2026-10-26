//! The credentials check, driven through its public surface against fakes.
//!
//! The check asks each curator who it is through the adapter in front of it, which
//! presents the key lemonfiber minted for it. The adapter is faked here as a transport
//! answering its contract as it would. The check is built on `#[async_trait]` clients
//! built on another, so — as with the VPN check — it is exercised from here rather than
//! from a `#[cfg(test)]` module, where async-trait code is compiled twice and its
//! coverage counted from the wrong copy.

use std::sync::Arc;

use lemonfiber_contract::capabilities::library::curate;
use lemonfiber_contract::wire::Kind;
use lemonfiber_core::doctor::credentials::{CredentialsCheck, Target};
use lemonfiber_core::doctor::{Category, Check, Verdict};
use lemonfiber_core::error::codes::cred::CREDENTIAL_REJECTED;
use lemonfiber_core::error::Severity;
use lemonfiber_core::ports::service::Identity;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_testing::adapted::{refused, Adapted, KEY};
use lemonfiber_testing::curators::{answered, curating, unavailable, FILM, TELEVISION};

/// What a healthy curator answers about itself.
fn sonarr() -> Identity {
    Identity {
        name: "Sonarr".to_owned(),
        version: "4.0.15.2941".to_owned(),
    }
}

/// `curator`, asked through its adapter answering `identity` as `answer`.
fn asked(curator: Adapted, name: &str, http: &Arc<Fake>) -> Target {
    Target {
        id: curator.plugin.to_owned(),
        name: name.to_owned(),
        kind: None,
        asked: Some(Arc::new(curate::Adapter(curator.contracted(http.clone())))),
    }
}

/// The television curator, its adapter answering `identity` as `answer`.
fn television(answer: Answer) -> (Target, Arc<Fake>) {
    let http = Fake::by_path(vec![(curating(TELEVISION, "identity"), answer)]);
    (asked(TELEVISION, "Sonarr", &http), http)
}

/// Run the check over one target.
async fn only(target: Target) -> Verdict {
    let check = CredentialsCheck::new(vec![target]);
    let mut findings = check.run().await;
    findings.pop().map_or(
        Verdict::Skipped {
            reason: String::new(),
        },
        |found| found.verdict,
    )
}

#[tokio::test]
async fn a_proven_credential_names_the_service_and_its_version() {
    let (target, _) = television(answered(&sonarr()));

    assert_eq!(
        only(target).await,
        Verdict::Pass {
            note: Some("Sonarr 4.0.15.2941".to_owned())
        },
        "a pass states the observed identity, not a bare tick"
    );
}

#[tokio::test]
async fn a_service_that_refuses_its_key_is_a_failure() {
    let (target, _) = television(refused(Kind::Unauthorised));

    let problem = match only(target).await {
        Verdict::Fail(problem) => Some(problem),
        _ => None,
    };
    assert!(
        problem.is_some_and(|problem| problem.code == CREDENTIAL_REJECTED
            && problem.severity == Severity::Error
            && !problem.remedies.is_empty()),
        "a rejected key must fail, as an error carrying a remedy"
    );
}

#[tokio::test]
async fn a_service_that_does_not_answer_is_unverified_rather_than_failed() {
    // Nothing answered, so the credential is unproven — never a pass, but not a
    // fault either: the honest verdict is that it could not be established.
    let (target, _) = television(unavailable());

    assert!(matches!(only(target).await, Verdict::Unverified { .. }));
}

#[tokio::test]
async fn a_service_answering_unusably_carries_its_own_words() {
    let refusal = lemonfiber_contract::wire::Refusal::new(Kind::Refused, "database is locked");
    let (target, _) = television(Answer::served(
        Kind::Refused.status(),
        lemonfiber_contract::wire::PROBLEM,
        serde_json::to_string(&refusal).unwrap_or_default(),
    ));

    let reason = match only(target).await {
        Verdict::Unverified { reason, .. } => Some(reason),
        _ => None,
    };
    assert!(
        reason.is_some_and(|reason| reason.contains("database is locked")),
        "an unusable answer is unverified, carrying the service's own words"
    );
}

#[tokio::test]
async fn a_service_on_an_unsupported_release_is_unverified_with_that_reason() {
    let (target, _) = television(refused(Kind::Unsupported));

    let reason = match only(target).await {
        Verdict::Unverified { reason, .. } => Some(reason),
        _ => None,
    };
    assert!(
        reason.is_some_and(|reason| reason.contains("API version this build speaks")),
        "an unsupported release is unverified, named as a version mismatch"
    );
}

#[tokio::test]
async fn a_curator_whose_adapter_cannot_be_asked_yet_is_skipped() {
    let target = Target {
        asked: None,
        ..television(answered(&sonarr())).0
    };

    let verdict = only(target).await;

    assert!(
        matches!(&verdict, Verdict::Skipped { reason } if reason.contains("Sonarr")),
        "{verdict:?}"
    );
}

#[tokio::test]
async fn each_service_is_reported_independently() {
    // One up, one not answering: the run reports both, and the unreachable one
    // does not stop the reachable one being proven.
    let http = Fake::by_path(vec![
        (curating(TELEVISION, "identity"), answered(&sonarr())),
        (curating(FILM, "identity"), unavailable()),
    ]);

    let check = CredentialsCheck::new(vec![
        asked(TELEVISION, "Sonarr", &http),
        asked(FILM, "Radarr", &http),
    ]);
    let findings = check.run().await;

    assert_eq!(findings.len(), 2);
    let curator_found = findings
        .iter()
        .find(|found| found.check == "credentials.sonarr");
    let second_curator_found = findings
        .iter()
        .find(|found| found.check == "credentials.radarr");
    assert!(
        curator_found.is_some_and(|found| matches!(found.verdict, Verdict::Pass { .. })),
        "the reachable service is proven"
    );
    assert!(
        second_curator_found
            .is_some_and(|found| matches!(found.verdict, Verdict::Unverified { .. })),
        "the unreachable one is unproven, and did not stop the other"
    );
}

#[tokio::test]
async fn nothing_to_prove_produces_no_findings() {
    let check = CredentialsCheck::new(Vec::new());
    assert!(check.run().await.is_empty());
}

#[tokio::test]
async fn the_check_belongs_to_the_credentials_category() {
    let check = CredentialsCheck::new(Vec::new());
    assert_eq!(check.category(), Category::Credentials);
}

/// Outcomes are reported, never inputs. Whatever the verdict, the key the adapter is
/// asked with must not be anywhere in what is shown.
#[tokio::test]
async fn the_credential_never_appears_in_a_finding() {
    for answer in [
        answered(&sonarr()),
        refused(Kind::Unauthorised),
        refused(Kind::Refused),
        unavailable(),
    ] {
        let (target, http) = television(answer);
        let check = CredentialsCheck::new(vec![target]);
        let rendered = serde_json::to_string(&check.run().await).unwrap_or_default();
        assert!(
            !rendered.contains(KEY),
            "the credential leaked into a finding"
        );
        assert!(
            http.requests()
                .iter()
                .all(|asked| asked.url.contains(&TELEVISION.port.to_string())),
            "the check asked somewhere other than the adapter"
        );
    }
}
