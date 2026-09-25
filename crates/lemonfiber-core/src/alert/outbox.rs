//! What has been said, what has not, and what is still owed.
//!
//! An alert is decided before it is delivered, and the two must not be the same
//! step. A channel that is down is exactly when something is worth saying, so a
//! design that only records what it managed to send loses precisely the alerts
//! that mattered most.
//!
//! So everything decided is written down first. Delivery marks it sent; a failure
//! leaves it owed and says so. Nothing is dropped because a channel was unreachable
//! at the moment it happened, and an operator who was away while a fault came and
//! went can still find out that it did.
//!
//! Pure: this decides what is owed to whom. Reaching a channel is the next layer's.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Alert;

/// How many delivered alerts are kept for the operator to read back.
///
/// Bounded because it is written between runs and nothing prunes it otherwise; a
/// history that grows without limit is a file that eventually costs more than it
/// tells anyone. Generous enough that a week of ordinary faults survives being
/// away from the machine.
pub const KEPT: usize = 200;

/// Everything owed to the operator, and everything already said.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outbox {
    /// Per check, the recurrence last delivered — what stops the same fault being
    /// reported twice, and what a digest is built against.
    #[serde(default)]
    told: BTreeMap<String, u32>,
    /// Decided and not yet delivered anywhere. Kept across runs, because a channel
    /// that was down is the case this exists for.
    #[serde(default)]
    owed: Vec<Alert>,
    /// What has been delivered, newest last, bounded. The in-app history, which
    /// needs no configuring and is where a fault that came and went while nobody
    /// was looking can still be found.
    #[serde(default)]
    said: Vec<Alert>,
}

impl Outbox {
    /// An outbox that has said nothing and owes nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The recurrence of this check the operator was last told about, which is what
    /// decides whether there is anything new to say.
    #[must_use]
    pub fn told(&self, check: &str) -> Option<u32> {
        self.told.get(check).copied()
    }

    /// Write alerts down as owed, before any attempt to deliver them.
    ///
    /// An alert already owed for the same check is replaced rather than repeated:
    /// what is owed is the current state of that check, not a queue of every state
    /// it passed through while a channel was down.
    pub fn owe(&mut self, alerts: impl IntoIterator<Item = Alert>) {
        for alert in alerts {
            // Matched on everything the new alert speaks for, so a group replaces
            // the individual alerts it now covers rather than sitting beside them.
            self.owed.retain(|owed| {
                !owed
                    .affected
                    .iter()
                    .any(|check| alert.affected.contains(check))
            });
            self.owed.push(alert);
        }
    }

    /// Everything waiting to be delivered.
    #[must_use]
    pub(crate) fn owing(&self) -> &[Alert] {
        &self.owed
    }

    /// Whether anything is waiting.
    #[must_use]
    pub fn owes_anything(&self) -> bool {
        !self.owed.is_empty()
    }

    /// Mark everything owed as delivered: it moves to the history, and each check's
    /// recurrence is recorded so the same fault is not reported again.
    ///
    /// `recurrence` answers "which spell of this check was this alert about?", since
    /// the alert carries the check and the condition carries the count.
    pub fn delivered(&mut self, recurrence: &dyn Fn(&str) -> u32) {
        for alert in std::mem::take(&mut self.owed) {
            // Every check the alert spoke for, not only the one it spoke in the
            // words of: a grouped alert has told the operator about all of them,
            // and leaving the rest unrecorded would report them again next run.
            for check in &alert.affected {
                self.told.insert(check.clone(), recurrence(check));
            }
            self.said.push(alert);
        }
        // Oldest first out, so what remains is the most recent history.
        if self.said.len() > KEPT {
            self.said.drain(..self.said.len() - KEPT);
        }
    }

    /// What has been delivered, newest first — the in-app history.
    #[must_use]
    pub fn history(&self) -> Vec<&Alert> {
        self.said.iter().rev().collect()
    }

    /// Forget a check entirely, for something that no longer exists to alert about.
    pub fn forget(&mut self, check: &str) {
        self.told.remove(check);
        self.owed
            .retain(|alert| !alert.affected.iter().any(|spoken| spoken == check));
    }
}

#[cfg(test)]
mod tests;
