//! What a surface saw, turned into the findings a summary is computed from.
//!
//! A surface reads containers and a tunnel; the summary reads conditions. This is
//! the one place that translation happens, so every surface agrees on what counts
//! as wrong and how much it matters — and so "the torrent client's traffic is not
//! behind the tunnel is critical" is written down once rather than at each caller.
//!
//! How much a service's failure matters comes from what the manifest says its
//! absence costs, not from the fact that a container is down. A failed subtitle
//! fetcher and a failed download client are not the same event, and grading them
//! alike is how an operator learns to ignore the summary.

use lemonfiber_manifest::Criticality;

use crate::condition::Condition;
use crate::docker::{Service, State};
use crate::error::Severity;

/// What the tunnel turned out to be doing, where a surface looked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Egress {
    /// This stack has no VPN-contained client, so there is nothing to be wrong.
    NotApplicable,
    /// The client's traffic was proven to leave through the tunnel.
    Behind,
    /// The client's traffic was proven to leave somewhere else.
    Leaking,
    /// It could not be established either way.
    Unreadable,
}

/// The conditions a surface's observations amount to.
///
/// Empty where nothing is wrong — which the summary reads as healthy only if the
/// stack was actually reachable, so an empty list from a machine nobody could ask
/// is not mistaken for a clean bill.
#[must_use]
pub fn observed(services: &[Service], egress: Egress, now: &str) -> Vec<Condition> {
    let mut raised = Vec::new();
    if let Some(condition) = tunnel(egress, now) {
        raised.push(condition);
    }
    raised.extend(
        services
            .iter()
            .filter(|service| service.state.wants_attention())
            .map(|service| service_condition(service, now)),
    );
    raised
}

/// The condition a tunnel amounts to.
///
/// A leak is critical — its consequence is outside the machine and cannot be
/// undone by stopping the stack afterwards. A tunnel that could not be read is a
/// warning rather than nothing: the reason to run a torrent client behind a VPN is
/// unverified, and reporting silence as safety is the failure this whole feature
/// exists to prevent.
fn tunnel(egress: Egress, now: &str) -> Option<Condition> {
    match egress {
        Egress::NotApplicable | Egress::Behind => None,
        Egress::Leaking => Some(Condition::raised(
            "vpn.egress",
            Severity::Critical,
            "the download client's traffic is not going through the tunnel",
            now,
        )),
        Egress::Unreadable => Some(Condition::raised(
            "vpn.egress",
            Severity::Warning,
            "whether the download client is behind the tunnel could not be established",
            now,
        )),
    }
}

/// The condition one service in a bad state amounts to.
fn service_condition(service: &Service, now: &str) -> Condition {
    Condition::raised(
        &format!("service.{}", service.id),
        severity_of(service.criticality),
        &summary_of(service),
        now,
    )
}

/// How much a service's failure matters, from what its absence costs.
///
/// The two least costly rungs are advisory: a failure confined to services that
/// only make things better is worth knowing and not worth waking anyone for.
const fn severity_of(criticality: Criticality) -> Severity {
    match criticality {
        Criticality::Critical => Severity::Critical,
        Criticality::Core => Severity::Error,
        Criticality::Important => Severity::Warning,
        Criticality::Enhancing | Criticality::Optional => Severity::Advisory,
    }
}

/// What is wrong with one service, in the operator's words rather than the
/// engine's — the same state renders differently depending on how it got there.
fn summary_of(service: &Service) -> String {
    let name = &service.name;
    match service.state {
        State::CrashLooping => format!("{name} keeps restarting"),
        State::Unhealthy => format!("{name} is running but its own check is failing"),
        _ => match service.exit {
            Some(code) => format!("{name} stopped on its own (exit {code})"),
            None => format!("{name} stopped on its own"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{observed, Egress};
    use crate::docker::{Service, State};
    use crate::error::Severity;
    use lemonfiber_manifest::Criticality;

    const NOW: &str = "2026-08-09T09:00:00Z";

    /// What was raised, as the pairs the assertions compare — whole lists rather
    /// than an element at a time, so an unexpected extra finding fails loudly
    /// instead of sitting unread behind an index.
    fn raised(services: &[Service], egress: Egress) -> Vec<(String, Severity)> {
        observed(services, egress, NOW)
            .into_iter()
            .map(|condition| (condition.check, condition.severity))
            .collect()
    }

    /// One service in a state, at a criticality.
    fn service(id: &str, state: State, criticality: Criticality) -> Service {
        Service {
            id: id.to_owned(),
            name: id.to_owned(),
            profile: "media".to_owned(),
            state,
            criticality,
            exit: None,
        }
    }

    #[test]
    fn a_stack_with_nothing_wrong_raises_nothing() {
        let services = [service("sonarr", State::Healthy, Criticality::Core)];
        assert!(observed(&services, Egress::Behind, NOW).is_empty());
        // Nor does a stack without a tunnel to be wrong about.
        assert!(observed(&services, Egress::NotApplicable, NOW).is_empty());
    }

    #[test]
    fn a_leaking_tunnel_is_critical_however_healthy_the_containers_are() {
        let services = [service("qbittorrent", State::Healthy, Criticality::Core)];
        assert_eq!(
            raised(&services, Egress::Leaking),
            vec![("vpn.egress".to_owned(), Severity::Critical)]
        );
    }

    #[test]
    fn a_tunnel_nobody_could_read_is_a_warning_rather_than_silence() {
        // Reporting "could not check" as safe is the failure the feature exists for.
        assert_eq!(
            raised(&[], Egress::Unreadable),
            vec![("vpn.egress".to_owned(), Severity::Warning)]
        );
    }

    #[test]
    fn how_much_a_failure_matters_comes_from_what_the_service_costs() {
        for (criticality, expected) in [
            (Criticality::Critical, Severity::Critical),
            (Criticality::Core, Severity::Error),
            (Criticality::Important, Severity::Warning),
            (Criticality::Enhancing, Severity::Advisory),
            (Criticality::Optional, Severity::Advisory),
        ] {
            let services = [service("x", State::Failed, criticality)];
            assert_eq!(
                raised(&services, Egress::NotApplicable),
                vec![("service.x".to_owned(), expected)],
                "{criticality:?}"
            );
        }
    }

    #[test]
    fn only_the_states_that_want_attention_raise_anything() {
        let services = [
            service("a", State::Healthy, Criticality::Core),
            service("b", State::Running, Criticality::Core),
            service("c", State::Starting, Criticality::Core),
            service("d", State::Stopped, Criticality::Core),
            service("e", State::Absent, Criticality::Core),
            service("f", State::HostManaged, Criticality::Core),
            service("g", State::Failed, Criticality::Core),
        ];
        assert_eq!(
            raised(&services, Egress::NotApplicable),
            vec![("service.g".to_owned(), Severity::Error)]
        );
    }

    #[test]
    fn each_bad_state_says_what_actually_happened() {
        let cases = [
            (State::CrashLooping, "sonarr keeps restarting"),
            (
                State::Unhealthy,
                "sonarr is running but its own check is failing",
            ),
            (State::Failed, "sonarr stopped on its own"),
        ];
        for (state, expected) in cases {
            let services = [service("sonarr", state, Criticality::Core)];
            let summaries: Vec<String> = observed(&services, Egress::NotApplicable, NOW)
                .into_iter()
                .map(|condition| condition.summary)
                .collect();
            assert_eq!(summaries, vec![expected.to_owned()], "{state:?}");
        }
    }

    #[test]
    fn an_exit_code_is_carried_where_the_engine_reported_one() {
        let failed = Service {
            exit: Some(137),
            ..service("sonarr", State::Failed, Criticality::Core)
        };
        let summaries: Vec<String> = observed(&[failed], Egress::NotApplicable, NOW)
            .into_iter()
            .map(|condition| condition.summary)
            .collect();
        assert_eq!(
            summaries,
            vec!["sonarr stopped on its own (exit 137)".to_owned()]
        );
    }

    #[test]
    fn the_tunnel_comes_before_the_containers() {
        // Not the ordering the summary presents — that sorts by severity — but the
        // one that keeps a leak from being dropped behind a long list of services.
        let services = [service("sonarr", State::Failed, Criticality::Core)];
        let checks: Vec<String> = raised(&services, Egress::Leaking)
            .into_iter()
            .map(|(check, _)| check)
            .collect();
        assert_eq!(checks, vec!["vpn.egress", "service.sonarr"]);
    }
}
