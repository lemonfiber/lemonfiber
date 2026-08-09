//! The one-line summary, computed once so every surface says the same thing.

use std::cmp::Reverse;

use serde::{Deserialize, Serialize};

use super::{Reach, Standing};
use crate::condition::Condition;
use crate::error::Severity;

/// The one-line summary, and what it expands to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    /// The one word.
    pub standing: Standing,
    /// How many things are wrong — the raised conditions, counted once each.
    pub wanting_attention: usize,
    /// The worst thing, named, so the line says something rather than only
    /// grading. Absent where nothing is wrong.
    pub worst: Option<String>,
    /// Everything that is wrong, worst first, so the line expands to the affected
    /// items rather than to a number the operator cannot act on.
    pub affected: Vec<Affected>,
}

/// One thing that is wrong, as the expanded summary lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Affected {
    /// The check that raised it, which is what a remedy is looked up by.
    pub check: String,
    /// How bad it is.
    pub severity: Severity,
    /// What is wrong, in one line.
    pub summary: String,
}

impl Summary {
    /// Summarise, from what is wrong and how far the stack got.
    ///
    /// How far the stack got settles only the cases where "wrong" has no meaning
    /// yet: nothing set up, nothing running on purpose, still coming up, or nobody
    /// could look. Everywhere else the conditions decide, because a container being
    /// up is not evidence that what it is doing is right.
    #[must_use]
    pub fn of(reach: Reach, raised: &[&Condition]) -> Self {
        let mut affected: Vec<Affected> = raised
            .iter()
            .map(|condition| Affected {
                check: condition.check.clone(),
                severity: condition.severity,
                summary: condition.summary.clone(),
            })
            .collect();
        // Worst first, and stably, so two things equally wrong keep the order the
        // checks raised them in rather than an arbitrary one that moves each refresh.
        affected.sort_by_key(|item| Reverse(item.severity));

        let worst = affected.first().map(|first| first.severity);
        let standing = Self::standing(reach, worst);
        Self {
            standing,
            wanting_attention: affected.len(),
            worst: affected.first().map(|first| first.summary.clone()),
            affected,
        }
    }

    /// The one word, from how far the stack got and the worst thing wrong with it.
    ///
    /// A critical finding outranks every reason to stay quiet: a leak while the
    /// stack is "starting" is still a leak.
    const fn standing(reach: Reach, worst: Option<Severity>) -> Standing {
        match (reach, worst) {
            (_, Some(Severity::Critical)) => Standing::Critical,
            (Reach::Unconfigured, _) => Standing::Unconfigured,
            (Reach::Unreachable | Reach::Starting, _) => Standing::Unknown,
            (Reach::Stopped, _) => Standing::Stopped,
            (Reach::Running, Some(severity)) => Standing::of(severity),
            (Reach::Running, None) => Standing::Healthy,
        }
    }

    /// The line itself.
    ///
    /// A healthy stack gets as clear a sentence as a broken one — "healthy" said
    /// plainly, not an absence of complaint the operator has to interpret.
    #[must_use]
    pub fn said(&self) -> String {
        let Some(worst) = &self.worst else {
            return self.standing.word().to_owned();
        };
        let others = self.wanting_attention.saturating_sub(1);
        if others == 0 {
            return format!("{} — {worst}", self.standing.word());
        }
        format!(
            "{} — {worst}, and {others} other{}",
            self.standing.word(),
            if others == 1 { "" } else { "s" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Reach, Standing, Summary};
    use crate::condition::Condition;
    use crate::error::Severity;

    /// One thing wrong, at a severity.
    fn wrong(check: &str, severity: Severity, summary: &str) -> Condition {
        Condition::raised(check, severity, summary, "2026-08-09T09:00:00Z")
    }

    #[test]
    fn a_running_stack_with_nothing_wrong_is_healthy_and_says_so() {
        // As clear a sentence as a broken one, rather than an absence of complaint
        // the operator has to interpret.
        let summary = Summary::of(Reach::Running, &[]);
        assert_eq!(summary.standing, Standing::Healthy);
        assert_eq!(summary.said(), "healthy");
        assert!(!summary.standing.wants_attention());
        assert!(summary.affected.is_empty());
    }

    #[test]
    fn everything_running_with_a_critical_finding_is_not_healthy() {
        // The case the whole feature exists for: sixteen containers up and answering
        // while traffic leaves outside the tunnel.
        let leak = wrong(
            "vpn.egress",
            Severity::Critical,
            "traffic is leaving the tunnel",
        );
        let summary = Summary::of(Reach::Running, &[&leak]);
        assert_eq!(summary.standing, Standing::Critical);
        assert!(summary.standing.wants_attention());
        assert_eq!(summary.said(), "critical — traffic is leaving the tunnel");
    }

    #[test]
    fn the_summary_takes_the_worst_and_never_an_average() {
        // Two advisories and one broken thing is a broken stack, not a middling one.
        let (a, b, c) = (
            wrong("a", Severity::Advisory, "a note"),
            wrong("b", Severity::Error, "the disk is full"),
            wrong("c", Severity::Advisory, "another note"),
        );
        let summary = Summary::of(Reach::Running, &[&a, &b, &c]);
        assert_eq!(summary.standing, Standing::Broken);
        assert_eq!(summary.wanting_attention, 3);
        assert_eq!(summary.said(), "broken — the disk is full, and 2 others");
    }

    #[test]
    fn the_summary_expands_to_the_affected_items_worst_first() {
        // A number an operator cannot act on is not an expansion; the items are.
        let (a, b, c) = (
            wrong("a", Severity::Advisory, "a note"),
            wrong("b", Severity::Error, "the disk is full"),
            wrong("c", Severity::Advisory, "another note"),
        );
        let summary = Summary::of(Reach::Running, &[&a, &b, &c]);
        let listed: Vec<(&str, Severity, &str)> = summary
            .affected
            .iter()
            .map(|item| (item.check.as_str(), item.severity, item.summary.as_str()))
            .collect();
        assert_eq!(
            listed,
            vec![
                ("b", Severity::Error, "the disk is full"),
                ("a", Severity::Advisory, "a note"),
                ("c", Severity::Advisory, "another note"),
            ],
            "worst first, and stable among equals so the list does not shuffle"
        );
    }

    #[test]
    fn a_stack_nobody_could_look_at_is_unknown_rather_than_healthy() {
        // Never reported as healthy: an unreachable engine is not evidence of health.
        for reach in [Reach::Unreachable, Reach::Starting] {
            let summary = Summary::of(reach, &[]);
            assert_eq!(summary.standing, Standing::Unknown, "{reach:?}");
            assert_ne!(summary.standing, Standing::Healthy);
        }
    }

    #[test]
    fn a_leak_while_starting_is_still_a_leak() {
        // A critical finding outranks every reason to stay quiet.
        let leak = wrong("vpn.egress", Severity::Critical, "leaking");
        assert_eq!(
            Summary::of(Reach::Starting, &[&leak]).standing,
            Standing::Critical
        );
    }

    #[test]
    fn a_stack_stopped_on_purpose_is_not_a_failure() {
        let summary = Summary::of(Reach::Stopped, &[]);
        assert_eq!(summary.standing, Standing::Stopped);
        assert!(!summary.standing.wants_attention());
        assert_eq!(summary.said(), "stopped");
    }

    #[test]
    fn a_machine_with_nothing_set_up_says_that_rather_than_anything_about_health() {
        let summary = Summary::of(Reach::Unconfigured, &[]);
        assert_eq!(summary.standing, Standing::Unconfigured);
        assert_eq!(summary.said(), "not set up");
    }

    #[test]
    fn one_other_thing_reads_as_one_rather_than_ones() {
        let (a, b) = (
            wrong("a", Severity::Error, "the disk is full"),
            wrong("b", Severity::Advisory, "a note"),
        );
        assert_eq!(
            Summary::of(Reach::Running, &[&a, &b]).said(),
            "broken — the disk is full, and 1 other"
        );
    }

    #[test]
    fn a_stopped_stack_with_something_wrong_still_reads_as_stopped() {
        // Its containers are down on purpose; a warning raised against them is not a
        // reason to call a deliberately stopped stack degraded.
        let stale = wrong("images.stale", Severity::Warning, "an image is out of date");
        assert_eq!(
            Summary::of(Reach::Stopped, &[&stale]).standing,
            Standing::Stopped
        );
    }
}
