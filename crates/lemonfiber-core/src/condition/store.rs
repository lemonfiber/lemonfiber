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

use super::{Condition, Fault};

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
    pub fn observe(&mut self, check: &str, wrong: Option<&Fault>, now: &str) {
        match (wrong, self.by_check.get_mut(check)) {
            (Some(fault), Some(condition)) => condition.raise(fault, now),
            (Some(fault), None) => {
                self.by_check
                    .insert(check.to_owned(), Condition::raised(check, fault, now));
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

    /// Every condition the store knows, raised or cleared.
    ///
    /// Alerting needs both: a resolution is news about something that is no longer
    /// raised, so a reader that saw only what is wrong now could never report one.
    #[must_use]
    pub fn all(&self) -> Vec<&Condition> {
        self.by_check.values().collect()
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

    /// Record that a repair was carried out and left the fault standing.
    ///
    /// Counted so a repair that is not working stops being offered. Separate from a
    /// recurrence, which says the problem came back on its own: this one says lemonfiber
    /// was wrong about the cause, and those want different answers.
    pub fn attempted(&mut self, check: &str) {
        if let Some(condition) = self.by_check.get_mut(check) {
            condition.attempts = condition.attempts.saturating_add(1);
        }
    }

    /// Forget the attempts against a check, a repair having put it right.
    ///
    /// The fault going away is what earns this rather than the repair having run: a count
    /// cleared on the strength of an attempt would never reach the limit it exists for.
    pub fn mended(&mut self, check: &str) {
        if let Some(condition) = self.by_check.get_mut(check) {
            condition.attempts = 0;
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
mod tests;
