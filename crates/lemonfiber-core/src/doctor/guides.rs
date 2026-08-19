//! Proving the quality guide source can be reached, so a stale sync is noticed.
//!
//! Recyclarr keeps quality profiles current by syncing community guides from an
//! upstream source on a schedule. When that source cannot be reached the sync
//! brings nothing, but the profiles already in place stay exactly as they are —
//! nothing falls back to unconfigured. This check reports when that currency
//! cannot be confirmed, before an operator wonders why a preset's refinements
//! never appeared.
//!
//! It probes the source's reachability from here, which only ever establishes one
//! thing honestly: that a clean answer came back now. That is a proxy — not proof
//! — for whether Recyclarr's own scheduled sync reached it (a different host, a
//! git fetch rather than this page, a different moment). So the check makes exactly
//! two claims: reachable (a pass), or could-not-confirm (unverified). It never
//! asserts the stack is degraded from a probe that cannot see the sync itself —
//! "could not check" stays its own thing, never dressed as a verdict about the
//! stack.

use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::error::Remedy;
use crate::ports::http::{Http, Method, Request};

/// The upstream repository Recyclarr syncs the community quality guides from,
/// probed for reachability rather than fetched. A hand-maintained literal, not
/// derived from Recyclarr's own configuration — it must be updated by hand if the
/// upstream ever moves, or the probe checks a source unrelated to what syncs.
const GUIDE_SOURCE: &str = "https://github.com/TRaSH-Guides/Guides";

/// Probes the quality guide source, so a sync whose currency cannot be confirmed is
/// surfaced rather than silently assumed fine.
pub struct GuidesCheck {
    http: Arc<dyn Http>,
}

impl GuidesCheck {
    /// A check that probes the guide source over `http`.
    #[must_use]
    pub fn new(http: Arc<dyn Http>) -> Self {
        Self { http }
    }
}

#[async_trait]
impl Check for GuidesCheck {
    fn category(&self) -> Category {
        Category::Services
    }

    async fn run(&self) -> Vec<Finding> {
        let request = Request {
            method: Method::Get,
            url: GUIDE_SOURCE.to_owned(),
            headers: Vec::new(),
            body: None,
        };
        let verdict = match self.http.send(&request).await {
            Ok(response) if response.is_success() => Verdict::Pass {
                note: Some("the quality guide source is reachable from here".to_owned()),
            },
            // Reached but not serving, or not reached at all: either way the probe
            // established only that it could not confirm the source is available, not
            // that the stack is degraded — so it is unverified, never a warning. The
            // profiles in place are untouched meanwhile.
            Ok(response) => cannot_confirm(&format!("the source answered {}", response.status)),
            Err(unreachable) => cannot_confirm(&unreachable.reason),
        };
        vec![Finding::in_category(
            Category::Services,
            "services.quality-guides",
            "Quality guide source",
            verdict,
        )]
    }
}

/// The verdict when the probe could not get a clean answer: whether the profiles can
/// stay current is unestablished, and the ones in place are kept meanwhile.
fn cannot_confirm(detail: &str) -> Verdict {
    Verdict::Unverified {
        reason: format!(
            "the quality guide source could not be confirmed reachable, so whether the \
             profiles can stay current is unknown: {detail}"
        ),
        remedy: Remedy::new(
            "Check this machine can reach the internet and that the upstream is up, then \
             check again; the profiles in place remain in force meanwhile",
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lemonfiber_fixtures::http::{Answer, Fake};

    use super::GuidesCheck;
    use crate::doctor::{Category, Check, Verdict};

    /// The single verdict the check produces for a given answer.
    async fn verdict(answer: Arc<Fake>) -> Option<Verdict> {
        GuidesCheck::new(answer)
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

    #[test]
    fn the_check_is_a_services_check() {
        assert_eq!(
            GuidesCheck::new(answering(200)).category(),
            Category::Services
        );
    }
}
