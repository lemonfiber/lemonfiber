//! Every condition, kept between runs.
//!
//! One pass over the checks decides the whole store: each check either reports
//! something wrong, which raises its condition, or reports nothing, which clears
//! it. That is deliberately the same shape as the drift baseline — a store whose
//! only writer is a comparison — because the alternative, letting each check
//! raise and clear on its own, is how a store ends up holding conditions no
//! check remembers raising.
//!
//! A check that could not run at all clears nothing. "I could not tell" is not
//! "it is fine", and a store that forgot a fault because the checker was offline
//! would be the comfortable falsehood the trust features exist to remove.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Condition;
use crate::error::Severity;

/// Every condition this machine has raised, by the check that raised it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conditions {
    /// Keyed by check, so a condition and its finding cannot become two names for
    /// one problem. Ordered, so the file is stable between runs and a diff of it
    /// shows what changed rather than what moved.
    #[serde(default)]
    by_check: BTreeMap<String, Condition>,
}

impl Conditions {
    /// An empty store — a machine nothing has ever been wrong on.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record what a check found: `wrong` where it is, nothing where it is not.
    ///
    /// A check that could not be run does not call this at all. Passing `None` for
    /// an unrunnable check would clear a fault nobody proved was gone.
    pub fn observe(&mut self, check: &str, wrong: Option<(Severity, &str)>, now: &str) {
        match (wrong, self.by_check.get_mut(check)) {
            (Some((severity, summary)), Some(condition)) => {
                condition.raise(severity, summary, now);
            }
            (Some((severity, summary)), None) => {
                self.by_check.insert(
                    check.to_owned(),
                    Condition::raised(check, severity, summary, now),
                );
            }
            // Nothing wrong, and nothing was: there is no condition to write, and
            // inventing a cleared one would fill the store with things that never
            // happened.
            (None, Some(condition)) => condition.clear(now),
            (None, None) => {}
        }
    }

    /// What is wrong right now, worst first, and by check within a severity so the
    /// order is the same on every run.
    #[must_use]
    pub fn raised(&self) -> Vec<&Condition> {
        let mut raised: Vec<&Condition> = self
            .by_check
            .values()
            .filter(|condition| condition.is_raised())
            .collect();
        raised.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.check.cmp(&b.check))
        });
        raised
    }

    /// One condition, raised or cleared, where the store knows it.
    #[must_use]
    pub fn get(&self, check: &str) -> Option<&Condition> {
        self.by_check.get(check)
    }

    /// Record that the operator declined a fix for this, so it stops being
    /// offered until the condition clears and genuinely comes back.
    pub fn decline(&mut self, check: &str) {
        if let Some(condition) = self.by_check.get_mut(check) {
            condition.declined = true;
        }
    }

    /// Forget a check entirely — what removing the thing it watched over means.
    /// A provider that is gone should not keep reporting that it is unreachable.
    pub fn forget(&mut self, check: &str) {
        self.by_check.remove(check);
    }

    /// Whether anything has ever been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_check.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::Conditions;
    use crate::error::Severity;

    /// A store with one stalled queue raised at a fixed moment.
    fn stalled() -> Conditions {
        let mut conditions = Conditions::new();
        conditions.observe(
            "queue.stalled",
            Some((Severity::Warning, "two downloads have not moved")),
            "2026-08-08T09:00:00Z",
        );
        conditions
    }

    #[test]
    fn a_check_that_finds_something_wrong_raises_it() {
        let conditions = stalled();
        assert_eq!(conditions.raised().len(), 1);
        assert!(conditions
            .get("queue.stalled")
            .is_some_and(super::Condition::is_raised));
    }

    #[test]
    fn a_standing_fault_seen_again_is_the_same_condition() {
        // Every run re-raises what is still wrong; that must land on the condition
        // already there rather than starting a second one, or "since" would mean
        // "since the last time anyone looked".
        let mut conditions = stalled();
        conditions.observe(
            "queue.stalled",
            Some((Severity::Error, "and now the disk is full")),
            "2026-08-08T17:00:00Z",
        );
        assert_eq!(conditions.raised().len(), 1, "one problem, not two");
        let condition = conditions.get("queue.stalled");
        assert_eq!(
            condition.map(|c| (c.since.as_str(), c.severity, c.recurrences)),
            Some(("2026-08-08T09:00:00Z", Severity::Error, 0)),
            "since it broke, at the severity it has become"
        );
    }

    #[test]
    fn a_check_that_finds_nothing_clears_what_it_raised() {
        // The self-resolving stall: it went away on its own, and that is recorded
        // rather than silently forgotten.
        let mut conditions = stalled();
        conditions.observe("queue.stalled", None, "2026-08-08T10:00:00Z");
        assert!(conditions.raised().is_empty());
        let cleared = conditions
            .get("queue.stalled")
            .and_then(|condition| condition.cleared.clone());
        assert_eq!(cleared.as_deref(), Some("2026-08-08T10:00:00Z"));
    }

    #[test]
    fn a_healthy_check_that_never_failed_writes_nothing() {
        // Inventing a cleared condition for every passing check would fill the
        // store with things that never happened.
        let mut conditions = Conditions::new();
        conditions.observe("vpn.egress-match", None, "2026-08-08T09:00:00Z");
        assert!(conditions.is_empty());
    }

    #[test]
    fn what_is_wrong_is_listed_worst_first_and_stably_within_a_severity() {
        let mut conditions = Conditions::new();
        for (check, severity) in [
            ("b.warn", Severity::Warning),
            ("a.critical", Severity::Critical),
            ("a.warn", Severity::Warning),
            ("c.error", Severity::Error),
        ] {
            conditions.observe(check, Some((severity, "wrong")), "now");
        }
        let order: Vec<&str> = conditions
            .raised()
            .iter()
            .map(|condition| condition.check.as_str())
            .collect();
        assert_eq!(order, vec!["a.critical", "c.error", "a.warn", "b.warn"]);
    }

    #[test]
    fn a_declined_fix_is_remembered_against_its_condition() {
        let mut conditions = stalled();
        conditions.decline("queue.stalled");
        assert!(conditions
            .get("queue.stalled")
            .is_some_and(|condition| condition.declined));
        // Declining something the store has never heard of is not a failure; there
        // is simply nothing to record it against.
        conditions.decline("nothing.here");
        assert_eq!(conditions.get("nothing.here"), None);
    }

    #[test]
    fn forgetting_a_check_removes_it_rather_than_clearing_it() {
        // A provider that has been removed should not keep reporting that it is
        // unreachable, nor leave a cleared record implying it recovered.
        let mut conditions = stalled();
        conditions.forget("queue.stalled");
        assert_eq!(conditions.get("queue.stalled"), None);
        assert!(conditions.is_empty());
    }

    #[test]
    fn the_store_round_trips_through_its_serialised_form() {
        // It is the only memory of what was wrong before this run.
        let conditions = stalled();
        let text = serde_json::to_string(&conditions).unwrap_or_default();
        assert_eq!(
            serde_json::from_str::<Conditions>(&text).ok(),
            Some(conditions)
        );
        // And a file written before this field existed reads as an empty store
        // rather than failing the run.
        assert_eq!(
            serde_json::from_str::<Conditions>("{}").ok(),
            Some(Conditions::new())
        );
    }
}
