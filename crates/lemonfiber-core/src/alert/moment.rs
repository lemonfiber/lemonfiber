//! One thing worth telling the operator, and which way it went.

use serde::{Deserialize, Serialize};

use crate::condition::Condition;
use crate::error::Severity;

/// Which way a condition went.
///
/// Both directions are worth saying and neither is worth saying twice. An operator
/// told a disk filled up and never told it was resolved goes on believing it — so
/// resolution is an alert in its own right rather than the absence of one.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Alert {
    /// The check this came from, so an alert and its condition cannot drift apart.
    /// Where several were grouped, the first of them.
    pub check: String,
    /// What kind of event it is, shared by every instance of it.
    pub kind: String,
    /// Which way it went.
    pub moment: Moment,
    /// How much it matters. A resolution takes the severity of what resolved,
    /// because "the critical thing is over" is itself worth the attention the
    /// critical thing had.
    pub severity: Severity,
    /// What happened, in the words the condition was raised with.
    pub summary: String,
    /// What it costs the operator, which is the half between the event and the
    /// fix. "The tunnel dropped" and "restart the gateway" leave whoever reads
    /// them to work out for themselves whether anything leaked.
    pub meaning: String,
    /// What to do about it, most likely first. An alert that says what happened
    /// and not what to do is a notification, which is a different and worse thing.
    pub remedies: Vec<String>,
    /// Every check this alert speaks for, the first being [`Self::check`]. More
    /// than one where the same event was grouped across several services.
    pub affected: Vec<String>,
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
            kind: condition.kind.clone(),
            moment,
            severity: condition.severity,
            summary: condition.summary.clone(),
            meaning: condition.meaning.clone(),
            remedies: condition.remedies.clone(),
            affected: vec![condition.check.clone()],
        })
    }

    /// Whether this is loud enough to interrupt someone who asked for quiet.
    ///
    /// Only the critical, and only on the way in. A resolution is good news and can
    /// wait for morning; a leak cannot.
    #[must_use]
    pub(crate) const fn overrides_quiet(&self) -> bool {
        matches!(self.severity, Severity::Critical) && matches!(self.moment, Moment::Onset)
    }

    /// The line an operator reads.
    ///
    /// Where the alert speaks for several services, it says so rather than naming
    /// one and leaving the rest to be discovered separately.
    #[must_use]
    pub fn said(&self) -> String {
        let others = self.affected.len().saturating_sub(1);
        if others == 0 {
            return format!("{} — {}", self.summary, self.moment.said());
        }
        format!(
            "{} — {}, and {others} other service{}",
            self.summary,
            self.moment.said(),
            crate::plural::s(others)
        )
    }
}

#[cfg(test)]
mod tests;
