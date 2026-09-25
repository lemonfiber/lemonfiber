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
pub(crate) fn suppressing(findings: Vec<Finding>, accepted: &Accepted) -> Vec<Finding> {
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
mod tests;
