//! Several things at once, said once.
//!
//! Two rules, both about not interrupting somebody six times in a second.
//!
//! A stack coming apart produces one alert per thing, and six alerts arriving
//! together are read as six emergencies rather than one bad minute. They are one
//! message, worst first, because the worst is what to act on.
//!
//! And a service flapping between broken and working produces an alert each way,
//! for ever. Past a few round trips the useful thing to say is that it is
//! flapping — which is a different fault, with a different remedy, and saying it
//! forty times as two alternating states says neither.

use serde::{Deserialize, Serialize};

use super::{Alert, Moment};
use crate::condition::Condition;
use crate::error::Severity;

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
    /// The digest for a set of conditions, given what the operator was last told
    /// about each.
    ///
    /// `told` answers "which recurrence of this check have they already heard
    /// about?" — absent means never.
    #[must_use]
    pub fn of<'a>(
        conditions: impl IntoIterator<Item = &'a Condition>,
        told: &dyn Fn(&str) -> Option<u32>,
    ) -> Self {
        let mut alerts: Vec<Alert> = conditions
            .into_iter()
            .filter_map(|condition| alert_for(condition, told(&condition.check)))
            .collect();
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
    pub fn overrides_quiet(&self) -> bool {
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
            if rest == 1 { "" } else { "s" }
        ))
    }
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
        moment: Moment::Onset,
        severity: condition.severity,
        summary: format!(
            "{} — and has come back {} times",
            condition.summary, condition.recurrences
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{Digest, FLAPPING};
    use crate::condition::Condition;
    use crate::error::Severity;

    /// A condition that has come back `times` times and is wrong now.
    fn flapped(check: &str, severity: Severity, times: u32) -> Condition {
        let mut condition = Condition::raised(check, severity, "it broke", "t0");
        for n in 0..times {
            condition.clear("t1");
            condition.raise(severity, "it broke", &format!("t{}", n + 2));
        }
        condition
    }

    /// Nobody has been told anything.
    fn untold(_check: &str) -> Option<u32> {
        None
    }

    #[test]
    fn six_things_at_once_are_one_message_worst_first() {
        // Six alerts arriving together read as six emergencies rather than one bad
        // minute, and the worst is what to act on.
        let conditions = vec![
            flapped("b.warn", Severity::Warning, 0),
            flapped("a.critical", Severity::Critical, 0),
            flapped("c.error", Severity::Error, 0),
        ];
        let digest = Digest::of(&conditions, &untold);
        assert_eq!(
            digest
                .alerts
                .iter()
                .map(|a| a.check.as_str())
                .collect::<Vec<_>>(),
            vec!["a.critical", "c.error", "b.warn"]
        );
        assert_eq!(digest.worst(), Some(Severity::Critical));
    }

    #[test]
    fn two_of_one_severity_are_ordered_by_check_so_two_runs_read_alike() {
        // Without a tie-break the order is whatever the source iterated in, and a
        // digest that shuffles between runs is one nobody can scan.
        let second = flapped("b.second", Severity::Error, 0);
        let first = flapped("a.first", Severity::Error, 0);
        let digest = Digest::of([&second, &first], &untold);
        assert_eq!(
            digest
                .alerts
                .iter()
                .map(|a| a.check.as_str())
                .collect::<Vec<_>>(),
            vec!["a.first", "b.second"]
        );
    }

    #[test]
    fn a_digest_says_the_worst_and_counts_the_rest() {
        let critical = flapped("a.critical", Severity::Critical, 0);
        let warning = flapped("b.warn", Severity::Warning, 0);

        let two = Digest::of([&critical, &warning], &untold).headline();
        assert_eq!(two.as_deref(), Some("it broke — started (and 1 other)"));

        let one = Digest::of([&critical], &untold).headline();
        assert_eq!(one.as_deref(), Some("it broke — started"));

        let three = Digest::of(
            [&critical, &warning, &flapped("c.error", Severity::Error, 0)],
            &untold,
        )
        .headline();
        assert_eq!(three.as_deref(), Some("it broke — started (and 2 others)"));
    }

    #[test]
    fn nothing_worth_saying_is_an_empty_digest() {
        let digest = Digest::of(&[], &untold);
        assert!(digest.is_empty());
        assert_eq!(digest.headline(), None);
        assert_eq!(digest.worst(), None);
        assert!(!digest.overrides_quiet());
    }

    #[test]
    fn a_flapping_service_is_reported_as_flapping_rather_than_as_each_flap() {
        // Once is an incident, twice is bad luck; by the third round trip the pattern
        // is the fault, and it has a different remedy from either state.
        let condition = flapped("service.health", Severity::Warning, FLAPPING);
        let digest = Digest::of([&condition], &untold);
        let said = digest.headline().unwrap_or_default();
        assert!(said.contains("come back"), "{said}");
        assert!(said.contains(&FLAPPING.to_string()), "{said}");
    }

    #[test]
    fn a_flapping_service_already_reported_stays_quiet() {
        // The whole point: not one alert per flap, for ever.
        let condition = flapped("service.health", Severity::Warning, FLAPPING + 2);
        let heard = |_: &str| Some(FLAPPING);
        assert!(Digest::of([&condition], &heard).is_empty());
    }

    #[test]
    fn one_critical_carries_the_whole_digest_through_a_quiet_period() {
        // Splitting it would mean the emergency arrives without its context.
        let critical = flapped("a.critical", Severity::Critical, 0);
        let warning = flapped("b.warn", Severity::Warning, 0);
        assert!(Digest::of([&critical, &warning], &untold).overrides_quiet());
        assert!(!Digest::of([&warning], &untold).overrides_quiet());
    }
}
