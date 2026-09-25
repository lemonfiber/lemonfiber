use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};

use super::GuidesCheck;
use crate::doctor::{Category, Check, Verdict};

/// The single verdict the check produces for a given answer.
async fn verdict(answer: Arc<Fake>) -> Option<Verdict> {
    GuidesCheck::new(answer, true)
        .run()
        .await
        .into_iter()
        .next()
        .map(|finding| finding.verdict)
}

/// A transport answering every request with this status and an empty body.
fn answering(status: u16) -> Arc<Fake> {
    Fake::always(Answer::reply(status, ""))
}

#[tokio::test]
async fn a_reachable_guide_source_passes() {
    assert!(matches!(
        verdict(answering(200)).await,
        Some(Verdict::Pass { .. })
    ));
}

#[tokio::test]
async fn a_source_that_answers_an_error_is_unverified_not_a_verdict_on_the_stack() {
    // The probe reached the source but got an error, so it could not confirm the
    // source is available — that is unverified, never a claim the stack is degraded.
    assert!(matches!(
        verdict(answering(503)).await,
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn an_unreachable_source_is_unverified_not_a_warning_or_failure() {
    // "Could not reach it from here" is exactly what Unverified is for: the probe
    // established nothing about whether the sync is stale, only that it could not
    // check — so it must not render as Warn (degraded) or Fail (broken).
    let unverified = verdict(Fake::silent()).await;
    assert!(matches!(unverified, Some(Verdict::Unverified { .. })));
}

/// The transport is handed in and must not be touched: a check that asked
/// anyway and then discarded the answer would pass this if it only read the
/// verdict.
#[tokio::test]
async fn a_source_the_operator_refuses_is_skipped_and_never_asked() {
    let answer = answering(200);
    let refused = GuidesCheck::new(answer.clone(), false)
        .run()
        .await
        .into_iter()
        .next()
        .map(|finding| finding.verdict);
    assert!(
        matches!(&refused, Some(Verdict::Skipped { reason })
            if reason.contains(crate::config::REACH_GUIDES_KEY)),
        "{refused:?}"
    );
    assert!(answer.requests().is_empty(), "the source was asked anyway");
}

#[test]
fn the_check_is_a_services_check() {
    assert_eq!(
        GuidesCheck::new(answering(200), true).category(),
        Category::Services
    );
}
