//! Several things at once, said once.
//!
//! Four rules, all about not interrupting somebody six times in a second.
//!
//! A stack coming apart produces one alert per thing, and six alerts arriving
//! together are read as six emergencies rather than one bad minute. They are one
//! message, worst first, because the worst is what to act on.
//!
//! Four services failing the same way is one event about four services, not four
//! events. The operator's next action is the same either way, and reading the same
//! sentence four times with a different name in it is how a digest gets skimmed.
//!
//! A service flapping between broken and working produces an alert each way, for
//! ever. Past a few round trips the useful thing to say is that it is flapping —
//! which is a different fault, with a different remedy, and saying it forty times
//! as two alternating states says neither.
//!
//! And a stack the operator deliberately stopped is not a stack that broke. Every
//! service being down is what they asked for, and reporting it as a fault teaches
//! them that stopping the stack means a page of alerts.
//!
//! On top of all of which sits what the operator actually asked to hear about,
//! which is the one rule here they chose rather than inherited.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{is_ours, Alert, Moment, Wants};
use crate::condition::Condition;
use crate::error::Severity;
use crate::health::Reach;

/// How many times a condition may come back before the flapping is the fault.
///
/// Three is a judgement, not a measurement: once is an incident, twice is bad
/// luck, and by the third round trip the pattern is the thing worth reporting.
pub const FLAPPING: u32 = 3;

/// Everything worth saying at one moment, as one message.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Digest {
    /// The alerts, worst first, then by check so two runs of one stack read alike.
    pub alerts: Vec<Alert>,
}

impl Digest {
    /// The digest for a set of conditions on a stack that is up, given what the
    /// operator was last told about each.
    ///
    /// `told` answers "which recurrence of this check have they already heard
    /// about?" — absent means never.
    #[must_use]
    pub fn of<'a>(
        conditions: impl IntoIterator<Item = &'a Condition>,
        told: &dyn Fn(&str) -> Option<u32>,
    ) -> Self {
        Self::reached(Reach::Running, conditions, told)
    }

    /// The digest for a stack that is up, for an operator with a default appetite.
    #[must_use]
    pub fn reached<'a>(
        reach: Reach,
        conditions: impl IntoIterator<Item = &'a Condition>,
        told: &dyn Fn(&str) -> Option<u32>,
    ) -> Self {
        Self::wanted(reach, &Wants::default(), conditions, told)
    }

    /// The digest, given how far the stack got and what the operator asked to hear.
    ///
    /// A stack the operator stopped on purpose says nothing operational: its
    /// services being down is what was asked for. What is not about the running
    /// stack — a channel that will not take deliveries — is still said, since that
    /// is wrong whatever the stack is doing.
    ///
    /// A resolution is delivered on the same terms as its onset, which is why the
    /// appetite is read from the condition rather than from the alert: an operator
    /// told a disk filled up and never told it was resolved goes on believing it.
    #[must_use]
    pub fn wanted<'a>(
        reach: Reach,
        wants: &Wants,
        conditions: impl IntoIterator<Item = &'a Condition>,
        told: &dyn Fn(&str) -> Option<u32>,
    ) -> Self {
        let mut alerts: Vec<Alert> = conditions
            .into_iter()
            .filter(|condition| is_ours(&condition.kind))
            .filter(|condition| wants.wants(&condition.kind, condition.severity))
            .filter(|condition| !is_expected_while_stopped(condition, reach))
            .filter_map(|condition| alert_for(condition, told(&condition.check)))
            .collect();
        alerts = group(alerts);
        alerts.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.check.cmp(&b.check))
        });
        Self { alerts }
    }

    /// Whether there is anything to send at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.alerts.is_empty()
    }

    /// Whether any of it is loud enough to interrupt a quiet period.
    ///
    /// One critical alert carries the whole digest through: splitting it to deliver
    /// half now and half later would mean the operator reads the emergency without
    /// the context arriving beside it.
    #[must_use]
    pub(crate) fn overrides_quiet(&self) -> bool {
        self.alerts.iter().any(Alert::overrides_quiet)
    }

    /// The worst thing in it, which is what a one-line summary should lead with.
    #[must_use]
    pub fn worst(&self) -> Option<Severity> {
        self.alerts.iter().map(|alert| alert.severity).max()
    }

    /// The whole digest as one line, for a channel with room for nothing more.
    #[must_use]
    pub fn headline(&self) -> Option<String> {
        let first = self.alerts.first()?;
        let rest = self.alerts.len().saturating_sub(1);
        if rest == 0 {
            return Some(first.said());
        }
        Some(format!(
            "{} (and {rest} other{})",
            first.said(),
            crate::plural::s(rest)
        ))
    }
}

/// Whether this is a fault the operator brought about by stopping the stack.
///
/// Only about the stack itself, and only while it is deliberately stopped — an
/// engine that could not be reached is not the same thing as one the operator
/// turned off, and a fault that has nothing to do with the containers running is
/// still a fault.
fn is_expected_while_stopped(condition: &Condition, reach: Reach) -> bool {
    reach == Reach::Stopped
        && OPERATIONAL
            .iter()
            .any(|kind| condition.kind.starts_with(kind))
}

/// The event domains that describe a running stack, and therefore say nothing
/// about one that is deliberately stopped.
const OPERATIONAL: [&str; 3] = ["service.", "vpn.", "queue."];

/// Fold alerts about the same event into one that names them all.
///
/// Grouped on the kind and which way it went, so "four services stopped" is one
/// alert and "two stopped while a third came back" is still two. The first by
/// check is the one that speaks, so the same set of services reads the same way on
/// every run rather than depending on what order the store was walked in.
fn group(alerts: Vec<Alert>) -> Vec<Alert> {
    let mut by_event: BTreeMap<(String, Moment), Alert> = BTreeMap::new();
    for alert in alerts {
        match by_event.entry((alert.kind.clone(), alert.moment)) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(alert);
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                let held = slot.get_mut();
                // The worst of them decides how loud the group is, and the earliest
                // by check decides which one it speaks in the words of.
                held.severity = held.severity.max(alert.severity);
                if alert.check < held.check {
                    held.check.clone_from(&alert.check);
                    held.summary.clone_from(&alert.summary);
                    held.meaning.clone_from(&alert.meaning);
                    held.remedies.clone_from(&alert.remedies);
                }
                held.affected.extend(alert.affected);
                held.affected.sort();
                held.affected.dedup();
            }
        }
    }
    by_event.into_values().collect()
}

/// The alert a condition earns, with flapping folded into one report of itself.
fn alert_for(condition: &Condition, told: Option<u32>) -> Option<Alert> {
    if condition.recurrences < FLAPPING {
        return Alert::of(condition, told);
    }
    // Past the threshold the states are noise and the pattern is the fault. Said
    // once: the operator has heard about this check at some recurrence already, and
    // hearing it again per flap is the thing being avoided.
    let unheard = told.is_none_or(|heard| heard < FLAPPING);
    unheard.then(|| Alert {
        check: condition.check.clone(),
        kind: condition.kind.clone(),
        moment: Moment::Onset,
        severity: condition.severity,
        summary: format!(
            "{} — and has come back {} times",
            condition.summary, condition.recurrences
        ),
        meaning: condition.meaning.clone(),
        remedies: condition.remedies.clone(),
        affected: vec![condition.check.clone()],
    })
}

#[cfg(test)]
mod tests;
