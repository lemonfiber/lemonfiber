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

mod fault;
mod store;

pub use fault::Fault;
pub use store::Conditions;

use serde::{Deserialize, Serialize};

use crate::error::Severity;

/// Something that is wrong, or was.
///
/// Keyed by the check that raised it, so the condition and the finding cannot
/// drift apart into two names for one problem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    /// The check this came from — `vpn.egress`, `service.sonarr`. Names the
    /// instance, which is what the store is keyed by.
    pub check: String,
    /// What kind of thing it is — `service.stopped`, `vpn.egress.leaking`. Shared
    /// by every instance of the same event, which is what lets four services
    /// stopping be one alert rather than four.
    #[serde(default)]
    pub kind: String,
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
    /// What to do about it, most likely first. Never empty, since the fault it was
    /// raised from could not have been built without one.
    #[serde(default)]
    pub remedies: Vec<String>,
    /// The check this one is downstream of, where it is known to be. Refreshed with
    /// the fault, because what something is caused by can change as the picture
    /// fills in.
    #[serde(default)]
    pub caused_by: Option<String>,
}

impl Condition {
    /// A condition raised now, for the first time.
    #[must_use]
    pub fn raised(check: &str, fault: &Fault, now: &str) -> Self {
        Self {
            check: check.to_owned(),
            kind: fault.kind.clone(),
            severity: fault.severity,
            summary: fault.summary.clone(),
            since: now.to_owned(),
            cleared: None,
            recurrences: 0,
            declined: false,
            remedies: fault.remedies.clone(),
            caused_by: fault.caused_by.clone(),
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
    ///
    /// The remedies and the cause are refreshed either way: what to do about a
    /// fault, and what it turns out to be downstream of, can both change as the
    /// picture fills in, and the stale answer is the wrong one to keep.
    pub fn raise(&mut self, fault: &Fault, now: &str) {
        self.kind.clone_from(&fault.kind);
        self.severity = fault.severity;
        self.summary.clone_from(&fault.summary);
        self.remedies.clone_from(&fault.remedies);
        self.caused_by.clone_from(&fault.caused_by);
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

    /// How long it has been clear, in seconds, as of `now`.
    ///
    /// `None` while it is still raised, and `None` where either stamp cannot be
    /// read — a store written by hand, or a clock that could not be reached.
    /// Unknown rather than a confident zero, because a caller deciding whether
    /// something has settled must be able to tell "not long" from "cannot say".
    #[must_use]
    pub fn settled_for(&self, now: &str) -> Option<u64> {
        let cleared: u64 = self.cleared.as_deref()?.parse().ok()?;
        let now: u64 = now.parse().ok()?;
        Some(now.saturating_sub(cleared))
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
    use super::{Condition, Fault};
    use crate::error::Severity;

    /// What the stall check reports, with what to do about it.
    fn stalled(summary: &str) -> Fault {
        Fault::new(
            "queue.stalled",
            Severity::Warning,
            summary,
            "check the indexer is answering",
        )
    }

    /// A condition raised at a fixed moment — the stamps are seconds since the
    /// epoch, which is what the clock port hands out.
    fn raised() -> Condition {
        Condition::raised(
            "queue.stalled",
            &stalled("two downloads have not moved in an hour"),
            "1000",
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
        condition.raise(&stalled("two downloads have not moved in an hour"), "2000");
        assert_eq!(condition.since, "1000");
        assert_eq!(condition.recurrences, 0, "it never went away");
    }

    #[test]
    fn a_standing_condition_still_takes_a_worsening() {
        // It has not gone away, but it has got worse, and the operator is owed the
        // worse one rather than the words it was first raised with.
        let mut condition = raised();
        let worse = Fault::new(
            "storage.full",
            Severity::Error,
            "the disk is full",
            "delete something",
        );
        condition.raise(&worse, "2000");
        assert_eq!(condition.severity, Severity::Error);
        assert_eq!(condition.summary, "the disk is full");
        assert_eq!(
            condition.remedies,
            vec!["delete something".to_owned()],
            "the remedy follows the fault, since the stale one is the wrong one"
        );
        assert_eq!(condition.since, "1000", "still since it broke");
    }

    #[test]
    fn a_condition_that_comes_back_is_a_recurrence_and_not_the_same_one() {
        // A fault that flaps is a different problem from one that has been steadily
        // broken, and only the count tells them apart.
        let mut condition = raised();
        condition.clear("1500");
        assert!(!condition.is_raised());
        assert_eq!(condition.cleared.as_deref(), Some("1500"));

        condition.raise(&stalled("and again"), "2000");
        assert!(condition.is_raised());
        assert_eq!(condition.recurrences, 1);
        assert_eq!(condition.since, "2000", "the new spell");
        assert_eq!(condition.cleared, None);
    }

    #[test]
    fn a_declined_fix_is_offered_again_only_once_the_problem_genuinely_returns() {
        // The difference between offering and nagging.
        let mut condition = raised();
        condition.declined = true;
        condition.raise(&stalled("still stalled"), "1500");
        assert!(
            condition.declined,
            "it never went away, so do not ask again"
        );

        condition.clear("2000");
        condition.raise(&stalled("stalled again"), "2500");
        assert!(!condition.declined, "it came back, so ask again");
    }

    #[test]
    fn clearing_what_is_already_clear_changes_nothing() {
        // A run over a healthy stack must not rewrite the store, or every run
        // produces a change for something that did not happen.
        let mut condition = raised();
        condition.clear("1500");
        let settled = condition.clone();
        condition.clear("2000");
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

        let advisory = Condition::raised(
            "x",
            &Fault::new("note", Severity::Advisory, "a note", "read it"),
            "1000",
        );
        assert!(!advisory.is_worth_saying(None), "not worth an interruption");

        let mut cleared = raised();
        cleared.clear("1500");
        assert!(!cleared.is_worth_saying(None), "nothing is wrong");
    }

    #[test]
    fn a_recurrence_is_worth_saying_even_where_the_first_spell_was_told() {
        // It came back; that is news, and the count is what makes it distinguishable
        // from the same standing fault.
        let mut condition = raised();
        condition.clear("1500");
        condition.raise(&stalled("again"), "2000");
        assert!(
            condition.is_worth_saying(Some(0)),
            "told about spell 0, this is 1"
        );
        assert!(!condition.is_worth_saying(Some(1)));
    }

    #[test]
    fn how_long_it_has_been_clear_is_read_from_the_two_stamps() {
        let mut condition = raised();
        assert_eq!(
            condition.settled_for("1090"),
            None,
            "still raised, so it has not settled for any time at all"
        );
        condition.clear("1000");
        assert_eq!(condition.settled_for("1090"), Some(90));
        // A clock that went backwards is no time at all rather than a negative one.
        assert_eq!(condition.settled_for("900"), Some(0));
        // A stamp that cannot be read is unknown, never a confident zero: a caller
        // has to be able to tell "not long" from "cannot say".
        assert_eq!(condition.settled_for("not a stamp"), None);
        condition.cleared = Some("yesterday".to_owned());
        assert_eq!(condition.settled_for("1090"), None);
    }

    #[test]
    fn a_store_written_before_remedies_existed_still_loads() {
        // The new fields default rather than failing the parse, so an existing
        // machine's store is not lost to an upgrade.
        let older = r#"{"check":"queue.stalled","severity":"warning","summary":"stalled",
            "since":"1000","cleared":null,"recurrences":0,"declined":false}"#;
        let parsed = serde_json::from_str::<Condition>(older).ok();
        assert_eq!(
            parsed.map(|condition| (condition.remedies, condition.caused_by)),
            Some((Vec::new(), None))
        );
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
