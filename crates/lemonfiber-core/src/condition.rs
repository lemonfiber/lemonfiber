//! What is wrong right now, remembered between runs.
//!
//! A check produces a finding: what is true at the moment it ran. That is enough
//! to print, and not enough for anything else the trust features need. "Stop
//! offering this fix until the condition clears and recurs" needs to know it was
//! ever raised. "The stall resolved itself" needs to know it used to be there.
//! "Notify at warning severity without being sought" needs to know this is new
//! rather than the same thing said again — an operator warned every run about the
//! same thing stops reading the warnings, which is worse than not warning.
//!
//! So a finding that fails raises a **condition**, which persists: when it started,
//! whether it has cleared, how many times it has come back. The word is the
//! specification's own — three features already speak of conditions clearing and
//! recurring rather than of findings.
//!
//! Nothing here reaches a service or a disk. Reading the store back and writing it
//! is the app layer's; what a condition *is*, and what raising and clearing one
//! mean, is here.

mod store;

pub use store::Conditions;

use serde::{Deserialize, Serialize};

use crate::error::Severity;

/// Something that is wrong, or was.
///
/// Keyed by the check that raised it, so the condition and the finding cannot
/// drift apart into two names for one problem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    /// The check this came from — `vpn.egress-match`, `queue.stalled`.
    pub check: String,
    /// How bad it is, in the same words every other severity uses.
    pub severity: Severity,
    /// What is wrong, in one line, as the finding said it.
    pub summary: String,
    /// When it was first raised. Untouched while it stays raised: an operator
    /// asking how long something has been broken means since it broke, not since
    /// it was last looked at.
    pub since: String,
    /// When it cleared, or nothing while it is still raised.
    pub cleared: Option<String>,
    /// How many times it has come back after clearing. A condition that flaps is
    /// a different problem from one that has been steadily broken, and only the
    /// count tells them apart.
    pub recurrences: u32,
    /// Whether the operator has declined a fix for it. Cleared when the condition
    /// clears, so a fix is offered again if the problem genuinely comes back —
    /// and not before, which is the difference between offering and nagging.
    pub declined: bool,
}

impl Condition {
    /// A condition raised now, for the first time.
    #[must_use]
    pub fn raised(check: &str, severity: Severity, summary: &str, now: &str) -> Self {
        Self {
            check: check.to_owned(),
            severity,
            summary: summary.to_owned(),
            since: now.to_owned(),
            cleared: None,
            recurrences: 0,
            declined: false,
        }
    }

    /// Whether this is wrong right now.
    #[must_use]
    pub const fn is_raised(&self) -> bool {
        self.cleared.is_none()
    }

    /// Raise it again, `now`.
    ///
    /// Already raised: nothing moves. The severity and summary are refreshed —
    /// a problem can worsen while it persists — but `since` is not, because how
    /// long something has been broken is measured from when it broke.
    ///
    /// Cleared and coming back: a recurrence. It starts again from now, the count
    /// goes up, and a previously declined fix is offered afresh.
    pub fn raise(&mut self, severity: Severity, summary: &str, now: &str) {
        self.severity = severity;
        summary.clone_into(&mut self.summary);
        if self.is_raised() {
            return;
        }
        self.cleared = None;
        now.clone_into(&mut self.since);
        self.recurrences = self.recurrences.saturating_add(1);
        self.declined = false;
    }

    /// Clear it, `now`. Clearing what is already clear changes nothing, so a run
    /// over a healthy stack does not rewrite the store.
    pub fn clear(&mut self, now: &str) {
        if self.is_raised() {
            self.cleared = Some(now.to_owned());
        }
    }

    /// Whether this is worth interrupting the operator about, given what they
    /// have already been told.
    ///
    /// Only a raised condition at warning or worse, and only while it is new —
    /// `told` is the last recurrence an operator was notified of. The same fault
    /// said every run is a fault an operator stops reading.
    #[must_use]
    pub fn is_worth_saying(&self, told: Option<u32>) -> bool {
        self.is_raised() && self.severity >= Severity::Warning && told != Some(self.recurrences)
    }
}

#[cfg(test)]
mod tests {
    use super::Condition;
    use crate::error::Severity;

    /// A condition raised at a fixed moment.
    fn raised() -> Condition {
        Condition::raised(
            "queue.stalled",
            Severity::Warning,
            "two downloads have not moved in an hour",
            "2026-08-08T09:00:00Z",
        )
    }

    #[test]
    fn a_raised_condition_is_wrong_right_now() {
        let condition = raised();
        assert!(condition.is_raised());
        assert_eq!(condition.recurrences, 0, "a first raise has not recurred");
        assert!(!condition.declined);
    }

    #[test]
    fn how_long_it_has_been_broken_is_measured_from_when_it_broke() {
        // An operator asking how long something has been wrong means since it went
        // wrong, not since it was last looked at — so a re-raise of a standing
        // condition must not restamp it.
        let mut condition = raised();
        condition.raise(
            Severity::Warning,
            "two downloads have not moved in an hour",
            "2026-08-08T17:00:00Z",
        );
        assert_eq!(condition.since, "2026-08-08T09:00:00Z");
        assert_eq!(condition.recurrences, 0, "it never went away");
    }

    #[test]
    fn a_standing_condition_still_takes_a_worsening() {
        // It has not gone away, but it has got worse, and the operator is owed the
        // worse one rather than the words it was first raised with.
        let mut condition = raised();
        condition.raise(Severity::Error, "the disk is full", "2026-08-08T17:00:00Z");
        assert_eq!(condition.severity, Severity::Error);
        assert_eq!(condition.summary, "the disk is full");
        assert_eq!(
            condition.since, "2026-08-08T09:00:00Z",
            "still since it broke"
        );
    }

    #[test]
    fn a_condition_that_comes_back_is_a_recurrence_and_not_the_same_one() {
        // A fault that flaps is a different problem from one that has been steadily
        // broken, and only the count tells them apart.
        let mut condition = raised();
        condition.clear("2026-08-08T10:00:00Z");
        assert!(!condition.is_raised());
        assert_eq!(condition.cleared.as_deref(), Some("2026-08-08T10:00:00Z"));

        condition.raise(Severity::Warning, "and again", "2026-08-08T11:00:00Z");
        assert!(condition.is_raised());
        assert_eq!(condition.recurrences, 1);
        assert_eq!(condition.since, "2026-08-08T11:00:00Z", "the new spell");
        assert_eq!(condition.cleared, None);
    }

    #[test]
    fn a_declined_fix_is_offered_again_only_once_the_problem_genuinely_returns() {
        // The difference between offering and nagging.
        let mut condition = raised();
        condition.declined = true;
        condition.raise(Severity::Warning, "still stalled", "2026-08-08T10:00:00Z");
        assert!(
            condition.declined,
            "it never went away, so do not ask again"
        );

        condition.clear("2026-08-08T11:00:00Z");
        condition.raise(Severity::Warning, "stalled again", "2026-08-08T12:00:00Z");
        assert!(!condition.declined, "it came back, so ask again");
    }

    #[test]
    fn clearing_what_is_already_clear_changes_nothing() {
        // A run over a healthy stack must not rewrite the store, or every run
        // produces a change for something that did not happen.
        let mut condition = raised();
        condition.clear("2026-08-08T10:00:00Z");
        let settled = condition.clone();
        condition.clear("2026-08-08T18:00:00Z");
        assert_eq!(condition, settled);
    }

    #[test]
    fn only_something_new_and_bad_enough_is_worth_interrupting_over() {
        // The same fault said every run is a fault an operator stops reading.
        let condition = raised();
        assert!(condition.is_worth_saying(None), "never told");
        assert!(
            !condition.is_worth_saying(Some(0)),
            "already told about this spell"
        );

        let advisory = Condition::raised("x", Severity::Advisory, "a note", "now");
        assert!(!advisory.is_worth_saying(None), "not worth an interruption");

        let mut cleared = raised();
        cleared.clear("2026-08-08T10:00:00Z");
        assert!(!cleared.is_worth_saying(None), "nothing is wrong");
    }

    #[test]
    fn a_recurrence_is_worth_saying_even_where_the_first_spell_was_told() {
        // It came back; that is news, and the count is what makes it distinguishable
        // from the same standing fault.
        let mut condition = raised();
        condition.clear("2026-08-08T10:00:00Z");
        condition.raise(Severity::Warning, "again", "2026-08-08T11:00:00Z");
        assert!(
            condition.is_worth_saying(Some(0)),
            "told about spell 0, this is 1"
        );
        assert!(!condition.is_worth_saying(Some(1)));
    }

    #[test]
    fn a_condition_round_trips_through_its_serialised_form() {
        // It is written between runs, so what comes back has to be what went in.
        let condition = raised();
        let text = serde_json::to_string(&condition).unwrap_or_default();
        assert_eq!(
            serde_json::from_str::<Condition>(&text).ok(),
            Some(condition)
        );
    }
}
