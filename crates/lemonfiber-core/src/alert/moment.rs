//! One thing worth telling the operator, and which way it went.

use serde::{Deserialize, Serialize};

use crate::condition::Condition;
use crate::error::Severity;

/// Which way a condition went.
///
/// Both directions are worth saying and neither is worth saying twice. An operator
/// told a disk filled up and never told it was resolved goes on believing it — so
/// resolution is an alert in its own right rather than the absence of one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Moment {
    /// It started.
    Onset,
    /// It stopped, having previously started.
    Resolved,
}

impl Moment {
    /// How this reads in front of what happened.
    #[must_use]
    pub const fn said(self) -> &'static str {
        match self {
            Self::Onset => "started",
            Self::Resolved => "resolved",
        }
    }
}

/// One interruption: what happened, which way, and how much it matters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alert {
    /// The check this came from, so an alert and its condition cannot drift apart.
    pub check: String,
    /// Which way it went.
    pub moment: Moment,
    /// How much it matters. A resolution takes the severity of what resolved,
    /// because "the critical thing is over" is itself worth the attention the
    /// critical thing had.
    pub severity: Severity,
    /// What happened, in the words the condition was raised with.
    pub summary: String,
}

impl Alert {
    /// The alert a condition earns right now, or nothing where it earns none.
    ///
    /// `told` is the recurrence an operator was last notified about, so a fault that
    /// is still the same fault says nothing. A condition that has cleared since they
    /// were told is a resolution, which is news.
    #[must_use]
    pub fn of(condition: &Condition, told: Option<u32>) -> Option<Self> {
        let moment = if condition.is_raised() {
            (told != Some(condition.recurrences)).then_some(Moment::Onset)
        } else {
            // Resolution is only news to somebody who heard about the onset.
            (told == Some(condition.recurrences)).then_some(Moment::Resolved)
        }?;
        Some(Self {
            check: condition.check.clone(),
            moment,
            severity: condition.severity,
            summary: condition.summary.clone(),
        })
    }

    /// Whether this is loud enough to interrupt someone who asked for quiet.
    ///
    /// Only the critical, and only on the way in. A resolution is good news and can
    /// wait for morning; a leak cannot.
    #[must_use]
    pub const fn overrides_quiet(&self) -> bool {
        matches!(self.severity, Severity::Critical) && matches!(self.moment, Moment::Onset)
    }

    /// The line an operator reads.
    #[must_use]
    pub fn said(&self) -> String {
        format!("{} — {}", self.summary, self.moment.said())
    }
}

#[cfg(test)]
mod tests {
    use super::{Alert, Moment};
    use crate::condition::{Condition, Fault};
    use crate::error::Severity;

    /// What a check reports, with something to do about it.
    fn wrong(severity: Severity, summary: &str) -> Fault {
        Fault::new(severity, summary, "look at it")
    }

    /// A condition raised at a fixed moment.
    fn raised() -> Condition {
        Condition::raised(
            "queue.stalled",
            &wrong(Severity::Warning, "two downloads have not moved"),
            "1000",
        )
    }

    #[test]
    fn something_newly_wrong_is_worth_an_interruption() {
        let alert = Alert::of(&raised(), None);
        assert_eq!(alert.as_ref().map(|a| a.moment), Some(Moment::Onset));
        assert_eq!(
            alert.map(|a| a.said()).as_deref(),
            Some("two downloads have not moved — started")
        );
    }

    #[test]
    fn the_same_fault_still_wrong_says_nothing() {
        // An operator told the same thing every run stops reading, which is worse
        // than never having been told.
        assert_eq!(Alert::of(&raised(), Some(0)), None);
    }

    #[test]
    fn a_resolution_is_news_to_whoever_heard_the_onset() {
        // Told a disk filled up and never told it was fixed, an operator goes on
        // believing it.
        let mut condition = raised();
        condition.clear("1000");
        let alert = Alert::of(&condition, Some(0));
        assert_eq!(alert.as_ref().map(|a| a.moment), Some(Moment::Resolved));
        assert_eq!(
            alert.map(|a| a.said()).as_deref(),
            Some("two downloads have not moved — resolved"),
            "and it reads as the good news it is"
        );
    }

    #[test]
    fn a_resolution_nobody_heard_the_onset_of_says_nothing() {
        // Something that broke and fixed itself between two runs never reached them,
        // and "it is better now" about a thing they never knew was worse is noise.
        let mut condition = raised();
        condition.clear("1000");
        assert_eq!(Alert::of(&condition, None), None);
    }

    #[test]
    fn a_resolution_carries_the_weight_of_what_resolved() {
        // "The critical thing is over" deserves the attention the critical thing had.
        let mut condition =
            Condition::raised("vpn.leak", &wrong(Severity::Critical, "leaking"), "1000");
        condition.clear("later");
        assert_eq!(
            Alert::of(&condition, Some(0)).map(|a| a.severity),
            Some(Severity::Critical)
        );
    }

    #[test]
    fn only_a_critical_onset_interrupts_someone_who_asked_for_quiet() {
        let critical = Condition::raised("vpn.leak", &wrong(Severity::Critical, "leaking"), "1000");
        assert!(Alert::of(&critical, None).is_some_and(|a| a.overrides_quiet()));

        // Good news can wait for morning.
        let mut over = critical.clone();
        over.clear("later");
        assert!(Alert::of(&over, Some(0)).is_some_and(|a| !a.overrides_quiet()));

        // And a warning is not an emergency however new it is.
        assert!(Alert::of(&raised(), None).is_some_and(|a| !a.overrides_quiet()));
    }
}
