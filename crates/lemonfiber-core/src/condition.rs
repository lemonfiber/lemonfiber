//! What is wrong right now, remembered between runs.
//!
//! A check produces a finding: what is true at the moment it ran. That is enough
//! to print, and not enough for anything else the trust features need. "Stop
//! offering this fix until the condition clears and recurs" needs to know it was
//! ever raised. "The stall resolved itself" needs to know it was there before.
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
    /// What it costs the operator, as the finding put it. Refreshed with the
    /// fault, since a problem's consequence can change as it worsens.
    ///
    /// Empty only for a condition written down before this was carried, and only
    /// until the check that raised it is next seen: raising refreshes it whether or
    /// not the fault is already standing, so a store from an older build fills
    /// itself in rather than needing a migration nobody would run.
    #[serde(default)]
    pub meaning: String,
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
    /// How many repairs have been carried out for it and left it standing.
    ///
    /// Counted separately from [`Self::recurrences`], which says how often the
    /// problem came back on its own. This says how often lemonfiber tried and was
    /// wrong about the cause, and a repair that has been wrong enough times stops
    /// being offered — an operator watching the same fix fail a fourth time is
    /// being wasted, not helped. Cleared with the rest when the condition clears,
    /// because a problem that genuinely returns deserves the attempt afresh.
    #[serde(default)]
    pub attempts: u32,
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
            meaning: fault.meaning.clone(),
            since: now.to_owned(),
            cleared: None,
            recurrences: 0,
            declined: false,
            attempts: 0,
            remedies: fault.remedies.clone(),
            caused_by: fault.caused_by.clone(),
        }
    }

    /// Whether this is wrong right now.
    #[must_use]
    pub(crate) const fn is_raised(&self) -> bool {
        self.cleared.is_none()
    }

    /// Raise it again, `now`.
    ///
    /// Already raised: nothing moves. The severity, the summary and what it means
    /// are refreshed — a problem can worsen while it persists — but `since` is not,
    /// because how long something has been broken is measured from when it broke.
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
        self.meaning.clone_from(&fault.meaning);
        self.remedies.clone_from(&fault.remedies);
        self.caused_by.clone_from(&fault.caused_by);
        if self.is_raised() {
            return;
        }
        self.cleared = None;
        now.clone_into(&mut self.since);
        self.recurrences = self.recurrences.saturating_add(1);
        self.declined = false;
        self.attempts = 0;
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
    pub(crate) fn settled_for(&self, now: &str) -> Option<u64> {
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
mod tests;
