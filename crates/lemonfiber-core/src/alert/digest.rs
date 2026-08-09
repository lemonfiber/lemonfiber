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

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{is_ours, Alert, Moment};
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

    /// The digest, given how far the stack got.
    ///
    /// A stack the operator stopped on purpose says nothing operational: its
    /// services being down is what was asked for. What is not about the running
    /// stack — a channel that will not take deliveries — is still said, since that
    /// is wrong whatever the stack is doing.
    #[must_use]
    pub fn reached<'a>(
        reach: Reach,
        conditions: impl IntoIterator<Item = &'a Condition>,
        told: &dyn Fn(&str) -> Option<u32>,
    ) -> Self {
        let mut alerts: Vec<Alert> = conditions
            .into_iter()
            .filter(|condition| is_ours(&condition.kind))
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
        remedies: condition.remedies.clone(),
        affected: vec![condition.check.clone()],
    })
}

#[cfg(test)]
mod tests {
    use super::{Digest, Moment, Reach, FLAPPING};
    use crate::condition::{Condition, Fault};
    use crate::error::Severity;

    /// A condition that has come back `times` times and is wrong now.
    fn flapped(check: &str, severity: Severity, times: u32) -> Condition {
        // Its own kind unless a test deliberately shares one, so a digest of
        // several checks is several alerts rather than one group.
        let fault = Fault::new(check, severity, "it broke", "look at it");
        let mut condition = Condition::raised(check, &fault, "1000");
        for n in 0..times {
            condition.clear("1100");
            condition.raise(&fault, &format!("{}", 1200 + n));
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

    // ── One event about four services ─────────────────────────────

    /// A service down, of a shared kind, so several of them are one event.
    fn stopped(check: &str, severity: Severity) -> Condition {
        Condition::raised(
            check,
            &Fault::new(
                "service.stopped",
                severity,
                &format!("{check} stopped on its own"),
                "start it again",
            ),
            "1000",
        )
    }

    #[test]
    fn four_services_failing_the_same_way_are_one_alert_naming_them_all() {
        // The operator's next action is the same either way, and reading the same
        // sentence four times with a different name in it is how a digest is skimmed.
        let services = ["service.sonarr", "service.radarr", "service.lidarr"];
        let raised: Vec<Condition> = services
            .iter()
            .map(|check| stopped(check, Severity::Error))
            .collect();
        let digest = Digest::of(raised.iter(), &untold);

        assert_eq!(digest.alerts.len(), 1, "one event, three services");
        let listed: Vec<(&str, Vec<&str>)> = digest
            .alerts
            .iter()
            .map(|alert| {
                (
                    alert.check.as_str(),
                    alert.affected.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(
            listed,
            vec![(
                "service.lidarr",
                vec!["service.lidarr", "service.radarr", "service.sonarr"]
            )],
            "the first by check speaks, so two runs of one stack read alike"
        );
        assert_eq!(
            digest.headline().as_deref(),
            Some("service.lidarr stopped on its own — started, and 2 other services")
        );
    }

    #[test]
    fn a_group_is_as_loud_as_the_worst_thing_in_it() {
        // Otherwise whichever service happened to sort first would decide how
        // seriously the operator takes a group containing something critical.
        let ordinary = stopped("service.aaa", Severity::Warning);
        let bad = stopped("service.zzz", Severity::Critical);
        let digest = Digest::of([&ordinary, &bad], &untold);
        assert_eq!(digest.worst(), Some(Severity::Critical));
        assert!(digest.overrides_quiet());
    }

    #[test]
    fn the_same_event_going_two_different_ways_stays_two_alerts() {
        // Two stopped and a third that came back is not one thing that happened.
        let down = stopped("service.aaa", Severity::Error);
        let mut back = stopped("service.zzz", Severity::Error);
        back.clear("1100");
        // Only the one that came back was ever reported, so only its resolution is
        // news; the other is still an onset nobody has heard.
        let heard = |check: &str| (check == "service.zzz").then_some(0);
        let moments: Vec<Moment> = Digest::of([&down, &back], &heard)
            .alerts
            .iter()
            .map(|alert| alert.moment)
            .collect();
        assert_eq!(moments, vec![Moment::Onset, Moment::Resolved]);
    }

    #[test]
    fn an_alert_carries_what_to_do_about_it() {
        // What happened without what to do is a notification, which is a different
        // and worse thing.
        let condition = stopped("service.sonarr", Severity::Error);
        let remedies: Vec<Vec<String>> = Digest::of([&condition], &untold)
            .alerts
            .iter()
            .map(|alert| alert.remedies.clone())
            .collect();
        assert_eq!(remedies, vec![vec!["start it again".to_owned()]]);
    }

    // ── Not ours to say, and not while stopped ────────────────────

    #[test]
    fn what_a_service_already_tells_its_own_users_is_never_repeated_here() {
        // A second message from lemonfiber is not an extra courtesy; it is what
        // teaches an operator to mute the channel that also carries the leak.
        let theirs = Condition::raised(
            "request.4231",
            &Fault::new(
                "request.available",
                Severity::Warning,
                "Dune is ready to watch",
                "open it",
            ),
            "1000",
        );
        assert!(Digest::of([&theirs], &untold).is_empty());
    }

    #[test]
    fn a_stack_the_operator_stopped_says_nothing_about_being_down() {
        // Every service being down is what was asked for. Reporting it as a fault
        // teaches the operator that stopping the stack means a page of alerts.
        let down = stopped("service.sonarr", Severity::Error);
        assert!(Digest::reached(Reach::Stopped, [&down], &untold).is_empty());
        assert!(!Digest::reached(Reach::Running, [&down], &untold).is_empty());
    }

    #[test]
    fn an_engine_nobody_could_reach_is_not_a_stack_somebody_turned_off() {
        // Suppressing on "not running" rather than "stopped on purpose" would go
        // quiet exactly when the machine stopped answering.
        let down = stopped("service.sonarr", Severity::Error);
        for reach in [Reach::Unreachable, Reach::Starting, Reach::Unconfigured] {
            assert!(
                !Digest::reached(reach, [&down], &untold).is_empty(),
                "{reach:?}"
            );
        }
    }

    #[test]
    fn a_channel_that_will_not_take_deliveries_is_reported_even_while_stopped() {
        // It is wrong whatever the stack is doing, and it is the reason the operator
        // may not have heard anything else.
        let refusing = Condition::raised(
            "notify.channel.discord",
            &Fault::new(
                "notify.channel.refused",
                Severity::Warning,
                "discord would not take it",
                "check the channel's configuration",
            ),
            "1000",
        );
        assert!(!Digest::reached(Reach::Stopped, [&refusing], &untold).is_empty());
    }
}
