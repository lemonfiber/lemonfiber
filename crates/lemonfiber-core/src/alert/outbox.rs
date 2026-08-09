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
            self.owed.retain(|owed| owed.check != alert.check);
            self.owed.push(alert);
        }
    }

    /// Everything waiting to be delivered.
    #[must_use]
    pub fn owing(&self) -> &[Alert] {
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
            self.told
                .insert(alert.check.clone(), recurrence(&alert.check));
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
        self.owed.retain(|alert| alert.check != check);
    }
}

#[cfg(test)]
mod tests {
    use super::{Outbox, KEPT};
    use crate::alert::{Alert, Moment};
    use crate::error::Severity;

    /// An alert about one check.
    fn alert(check: &str, moment: Moment) -> Alert {
        Alert {
            check: check.to_owned(),
            moment,
            severity: Severity::Warning,
            summary: "it broke".to_owned(),
        }
    }

    /// Every check is on its first spell.
    fn first(_check: &str) -> u32 {
        0
    }

    #[test]
    fn an_alert_is_owed_before_anything_tries_to_deliver_it() {
        // A channel that is down is exactly when something is worth saying, so
        // recording only what was sent loses the alerts that mattered most.
        let mut outbox = Outbox::new();
        outbox.owe([alert("queue.stalled", Moment::Onset)]);
        assert!(outbox.owes_anything());
        assert_eq!(outbox.owing().len(), 1);
        assert_eq!(outbox.told("queue.stalled"), None, "nothing said yet");
    }

    #[test]
    fn delivery_moves_it_to_the_history_and_stops_it_repeating() {
        let mut outbox = Outbox::new();
        outbox.owe([alert("queue.stalled", Moment::Onset)]);
        outbox.delivered(&first);

        assert!(!outbox.owes_anything());
        assert_eq!(outbox.told("queue.stalled"), Some(0));
        assert_eq!(outbox.history().len(), 1);
    }

    #[test]
    fn what_is_owed_is_the_current_state_and_not_every_state_it_passed_through() {
        // A channel down for an hour should not deliver forty alerts about one
        // check when it comes back; it should deliver where that check now stands.
        let mut outbox = Outbox::new();
        outbox.owe([alert("service.health", Moment::Onset)]);
        outbox.owe([alert("service.health", Moment::Resolved)]);
        assert_eq!(outbox.owing().len(), 1);
        assert_eq!(
            outbox.owing().first().map(|a| a.moment),
            Some(Moment::Resolved)
        );
    }

    #[test]
    fn two_checks_are_two_things_owed() {
        let mut outbox = Outbox::new();
        outbox.owe([alert("a", Moment::Onset), alert("b", Moment::Onset)]);
        assert_eq!(outbox.owing().len(), 2);
    }

    #[test]
    fn a_fault_that_came_and_went_unseen_is_still_in_the_history() {
        // The operator was away. It resolved. They can still find out it happened.
        let mut outbox = Outbox::new();
        outbox.owe([alert("disk.full", Moment::Onset)]);
        outbox.delivered(&first);
        outbox.owe([alert("disk.full", Moment::Resolved)]);
        outbox.delivered(&first);

        let history = outbox.history();
        assert_eq!(history.len(), 2);
        assert_eq!(
            history.first().map(|a| a.moment),
            Some(Moment::Resolved),
            "newest first"
        );
    }

    #[test]
    fn the_history_is_bounded_and_keeps_the_recent_end() {
        // It is written between runs and nothing else prunes it.
        let mut outbox = Outbox::new();
        for n in 0..KEPT + 50 {
            outbox.owe([alert(&format!("check.{n}"), Moment::Onset)]);
            outbox.delivered(&first);
        }
        assert_eq!(outbox.history().len(), KEPT);
        assert_eq!(
            outbox.history().first().map(|a| a.check.as_str()),
            Some(format!("check.{}", KEPT + 49).as_str()),
            "the newest survived"
        );
    }

    #[test]
    fn a_later_spell_is_recorded_as_a_later_spell() {
        // Delivering the second spell of a fault must not leave the outbox thinking
        // the operator has only heard about the first.
        let mut outbox = Outbox::new();
        outbox.owe([alert("queue.stalled", Moment::Onset)]);
        outbox.delivered(&|_| 2);
        assert_eq!(outbox.told("queue.stalled"), Some(2));
    }

    #[test]
    fn forgetting_a_check_drops_what_was_owed_and_what_was_told() {
        // A provider that has been removed should not be alerted about, nor keep a
        // record implying it was.
        let mut outbox = Outbox::new();
        outbox.owe([alert("provider.quota", Moment::Onset)]);
        outbox.delivered(&first);
        outbox.owe([alert("provider.quota", Moment::Onset)]);

        outbox.forget("provider.quota");
        assert_eq!(outbox.told("provider.quota"), None);
        assert!(!outbox.owes_anything());
    }

    #[test]
    fn the_outbox_round_trips_through_its_serialised_form() {
        // It is the only memory of what the operator has been told.
        let mut outbox = Outbox::new();
        outbox.owe([alert("a", Moment::Onset)]);
        outbox.delivered(&first);
        outbox.owe([alert("b", Moment::Onset)]);

        let text = serde_json::to_string(&outbox).unwrap_or_default();
        assert_eq!(serde_json::from_str::<Outbox>(&text).ok(), Some(outbox));
        // And a file written before any of these fields existed reads as empty.
        assert_eq!(
            serde_json::from_str::<Outbox>("{}").ok(),
            Some(Outbox::new())
        );
    }
}
