//! Not saying again what the operator has already answered.
//!
//! Some findings are about a choice rather than a fault. Running without a VPN,
//! or with a provider that forwards no port, is a decision with a cost — and
//! stating the cost is right, once. Stating it on every run afterwards is not a
//! second warning, it is the same warning, and an operator who has already
//! weighed it learns that this tool repeats itself. From there they stop reading
//! all of it, including the findings that are faults.
//!
//! So an acknowledged choice is suppressed rather than removed. The finding still
//! exists, still says what the cost is, and still appears where somebody asks to
//! see everything — it simply stops leading with it. The distinction matters
//! because "you chose this" and "this is not happening" are different claims, and
//! only one of them is true.
//!
//! Acknowledgement is its own record rather than the condition store's `declined`
//! flag. That flag means "stop offering this fix until the fault clears and comes
//! back", which is right for a fault and wrong for a choice: running without a
//! VPN never clears, so it would never be re-offered — but neither would it ever
//! be recorded, since nothing raises a condition about a decision the operator
//! made deliberately.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::doctor::{Finding, Verdict};
use crate::error::State;

/// The choices the operator has answered, by the check that would otherwise keep
/// asking.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accepted {
    #[serde(default)]
    checks: BTreeSet<String>,
}

impl Accepted {
    /// Nothing answered yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that the operator accepted this choice and its cost.
    pub fn accept(&mut self, check: &str) {
        self.checks.insert(check.to_owned());
    }

    /// Whether this choice has been answered.
    #[must_use]
    pub fn has(&self, check: &str) -> bool {
        self.checks.contains(check)
    }
}

/// Suppress the findings whose choice the operator has already answered.
///
/// Applied to a whole set rather than per finding, so the rule is in one place
/// and a surface cannot forget it for the one check where it matters most.
#[must_use]
pub fn suppressing(findings: Vec<Finding>, accepted: &Accepted) -> Vec<Finding> {
    findings
        .into_iter()
        .map(|finding| {
            if accepted.has(&finding.check) {
                suppress(finding)
            } else {
                finding
            }
        })
        .collect()
}

/// The same finding, marked as something already answered.
///
/// Kept as a warning rather than downgraded to a pass: nothing about the cost has
/// changed, and a surface that shows an acknowledged choice as fine would be
/// telling the operator something they can check and find untrue. The state is
/// what says it has been answered, and what a surface reads to decide whether to
/// lead with it.
fn suppress(finding: Finding) -> Finding {
    match finding.verdict {
        Verdict::Warn(problem) => Finding {
            verdict: Verdict::Warn(problem.in_state(State::Suppressed)),
            ..finding
        },
        // Only a warning is a choice with a cost. A failure is not something to
        // acknowledge away, and suppressing one would be the single most damaging
        // thing this could do.
        Verdict::Pass { .. }
        | Verdict::Fail(_)
        | Verdict::Unverified { .. }
        | Verdict::Skipped { .. } => finding,
    }
}

#[cfg(test)]
mod tests {
    use super::{suppressing, Accepted};
    use crate::doctor::{Category, Finding, Verdict};
    use crate::error::{Code, Problem, Remedy, Severity, State};

    const CODE: Code = Code::new("test.choice");

    /// Where each finding stands, as the surface reads it. Total rather than a
    /// match with a fallback, so the non-warning arm is exercised by the test
    /// about failures rather than left as a branch nothing reaches.
    fn states(findings: &[Finding]) -> Vec<Option<State>> {
        findings
            .iter()
            .map(|finding| match &finding.verdict {
                Verdict::Warn(problem) | Verdict::Fail(problem) => Some(problem.state),
                Verdict::Pass { .. } | Verdict::Unverified { .. } | Verdict::Skipped { .. } => None,
            })
            .collect()
    }

    /// A finding about a choice with a cost.
    fn warned(check: &str) -> Finding {
        Finding::in_category(
            Category::Vpn,
            check,
            "Running without a tunnel",
            Verdict::Warn(Problem::new(
                CODE,
                Severity::Warning,
                "No VPN is configured",
                "Torrent traffic leaves this machine under its own address.",
                Remedy::new("Configure a VPN, or accept this deliberately"),
            )),
        )
    }

    /// A record in which `check` has been answered.
    fn answered(check: &str) -> Accepted {
        let mut accepted = Accepted::new();
        accepted.accept(check);
        accepted
    }

    #[test]
    fn a_choice_already_answered_stops_leading() {
        // The same warning every run is not a second warning. An operator who has
        // weighed it learns the tool repeats itself, and stops reading all of it.
        let suppressed = suppressing(vec![warned("vpn.tunnel")], &answered("vpn.tunnel"));
        assert_eq!(states(&suppressed), vec![Some(State::Suppressed)]);
    }

    #[test]
    fn an_acknowledged_choice_is_never_shown_as_fine() {
        // "You chose this" and "this is not happening" are different claims, and
        // only one of them is true. It stays a warning; only its state moves.
        let suppressed = suppressing(vec![warned("vpn.tunnel")], &answered("vpn.tunnel"));
        assert!(suppressed
            .iter()
            .all(|finding| matches!(finding.verdict, Verdict::Warn(_))));
        // A pass would be telling the operator something they can check and find
        // untrue; only the state moves.
        assert_eq!(states(&suppressed), vec![Some(State::Suppressed)]);
    }

    #[test]
    fn a_choice_nobody_answered_is_left_exactly_as_it_was() {
        let untouched = suppressing(vec![warned("vpn.tunnel")], &Accepted::new());
        assert_eq!(states(&untouched), vec![Some(State::Actionable)]);
    }

    #[test]
    fn answering_one_choice_says_nothing_about_another() {
        let other = suppressing(vec![warned("vpn.port-forward")], &answered("vpn.tunnel"));
        assert_eq!(states(&other), vec![Some(State::Actionable)]);
    }

    #[test]
    fn a_failure_is_never_acknowledged_away() {
        // The single most damaging thing this could do. A fault is not a choice,
        // and no answer to it makes it stop being true.
        let failing = Finding::in_category(
            Category::Vpn,
            "vpn.egress",
            "Traffic is behind the tunnel",
            Verdict::Fail(Problem::new(
                CODE,
                Severity::Critical,
                "Traffic is leaving outside the tunnel",
                "Every torrent this machine runs is visible under its own address.",
                Remedy::new("Stop the download client"),
            )),
        );
        let after = suppressing(vec![failing], &answered("vpn.egress"));
        assert!(after
            .iter()
            .all(|finding| matches!(finding.verdict, Verdict::Fail(_))));
        // Still exactly where it was: an answer to a fault changes nothing about it.
        assert_eq!(states(&after), vec![Some(State::Actionable)]);
    }

    #[test]
    fn a_check_that_did_not_apply_is_left_alone_however_it_was_answered() {
        // Nothing was asked of the operator, so there is nothing they answered —
        // and a skipped check turned into a suppressed warning would claim a
        // decision nobody made.
        let skipped = Finding::in_category(
            Category::Vpn,
            "vpn.tunnel",
            "Running without a tunnel",
            Verdict::Skipped {
                reason: "this stack declares no torrent client".to_owned(),
            },
        );
        let after = suppressing(vec![skipped], &answered("vpn.tunnel"));
        assert_eq!(states(&after), vec![None]);
        assert!(after
            .iter()
            .all(|finding| matches!(finding.verdict, Verdict::Skipped { .. })));
    }
}
