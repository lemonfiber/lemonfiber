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
//!
//! Every check looked at is reported, including the ones that found nothing. The
//! condition store needs both: something wrong raises, nothing wrong clears, and a
//! check that could not be run says nothing at all — so a fault is never forgotten
//! merely because nobody looked this time.

use lemonfiber_manifest::Criticality;

use crate::condition::Fault;
use crate::docker::{Service, State};
use crate::error::Severity;

/// The check the tunnel's egress is filed under.
pub const EGRESS_CHECK: &str = "vpn.egress";

// The kinds of event this module raises. Named here rather than spelled at each
// site, since a kind is what an operator switches off and what groups four
// services failing alike into one alert.

/// The download client's traffic was proven to leave outside the tunnel.
pub const LEAKING: &str = "vpn.egress.leaking";
/// Whether it is behind the tunnel could not be established either way.
pub const UNVERIFIED: &str = "vpn.egress.unverified";
/// A service exited without being asked to.
pub const STOPPED: &str = "service.stopped";
/// A service is exiting and restarting repeatedly.
pub const CRASH_LOOPING: &str = "service.crash-looping";
/// A service is running and its own probe says it is not working.
pub const UNHEALTHY: &str = "service.unhealthy";

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

/// Every check this surface ran, and what it found.
///
/// `None` against a check means it ran and found nothing — which clears a standing
/// condition. A check that could not be run does not appear at all, because "I
/// could not tell" must never be recorded as "it is fine".
#[must_use]
pub fn observed(services: &[Service], egress: Egress) -> Vec<(String, Option<Fault>)> {
    let mut looked: Vec<(String, Option<Fault>)> = Vec::new();
    if egress != Egress::NotApplicable {
        looked.push((EGRESS_CHECK.to_owned(), tunnel(egress, services)));
    }
    looked.extend(
        services
            .iter()
            // Host-managed services are not lemonfiber's to start, so reporting one
            // as broken would blame it for something outside its control.
            .filter(|service| service.state != State::HostManaged)
            .map(|service| (check_of(service), service_fault(service, services))),
    );
    looked
}

/// The check one service is filed under.
fn check_of(service: &Service) -> String {
    format!("service.{}", service.id)
}

/// What the tunnel amounts to.
///
/// A leak is critical — its consequence is outside the machine and cannot be
/// undone by stopping the stack afterwards. A tunnel that could not be read is a
/// warning rather than nothing: the reason to run a torrent client behind a VPN is
/// unverified, and reporting silence as safety is the failure this whole feature
/// exists to prevent.
fn tunnel(egress: Egress, services: &[Service]) -> Option<Fault> {
    let fault = match egress {
        Egress::NotApplicable | Egress::Behind => return None,
        Egress::Leaking => Fault::new(
            LEAKING,
            Severity::Critical,
            "the download client's traffic is not going through the tunnel",
            "stop the download client until the tunnel is proven to carry its traffic",
        )
        .or_else("check the gateway container is running and connected"),
        Egress::Unreadable => Fault::new(
            UNVERIFIED,
            Severity::Warning,
            "whether the download client is behind the tunnel could not be established",
            "check the gateway container is running",
        )
        .or_else("set an IP-echo address so the egress can be compared"),
    };
    // A gateway that is itself down is why the tunnel cannot be trusted, rather
    // than a second thing wrong beside it.
    match services
        .iter()
        .find(|service| service.state.wants_attention() && is_gateway(service))
    {
        Some(gateway) => Some(fault.caused_by(&check_of(gateway))),
        None => Some(fault),
    }
}

/// Whether a service is what other services route their traffic through — the one
/// whose failure takes the tunnel with it.
fn is_gateway(service: &Service) -> bool {
    service.depends_on.is_empty() && service.criticality == Criticality::Critical
}

/// What is wrong with one service, or nothing where it is fine.
fn service_fault(service: &Service, services: &[Service]) -> Option<Fault> {
    if !service.state.wants_attention() {
        return None;
    }
    let fault = Fault::new(
        kind_of(service),
        severity_of(service.criticality),
        &summary_of(service),
        &remedy_of(service),
    )
    .or_else("read its logs for what it said before it stopped");

    // A service that cannot start because something it depends on is down is one
    // problem with the thing underneath, not two independent failures.
    match service.depends_on.iter().find(|needed| {
        services
            .iter()
            .any(|other| &&other.id == needed && other.state.wants_attention())
    }) {
        Some(needed) => Some(fault.caused_by(&format!("service.{needed}"))),
        None => Some(fault),
    }
}

/// Which kind of failure this is — what four services failing the same way have
/// in common, and what an operator switches off.
const fn kind_of(service: &Service) -> &'static str {
    match service.state {
        State::CrashLooping => CRASH_LOOPING,
        State::Unhealthy => UNHEALTHY,
        _ => STOPPED,
    }
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

/// The first thing to try, which differs by how the service failed: restarting
/// something that is already restarting itself achieves nothing.
fn remedy_of(service: &Service) -> String {
    let name = &service.name;
    match service.state {
        State::CrashLooping => {
            format!("{name} is restarting itself and failing; its configuration is the usual cause")
        }
        State::Unhealthy => format!("give {name} a moment, then restart it if it does not settle"),
        _ => format!("start {name} again"),
    }
}

#[cfg(test)]
mod tests {
    use super::{observed, Egress};
    use crate::docker::{Service, State};
    use crate::error::Severity;
    use lemonfiber_manifest::Criticality;

    /// What was found wrong, as the pairs the assertions compare — whole lists
    /// rather than an element at a time, so an unexpected extra finding fails
    /// loudly instead of sitting unread behind an index.
    fn wrong(services: &[Service], egress: Egress) -> Vec<(String, Severity)> {
        observed(services, egress)
            .into_iter()
            .filter_map(|(check, fault)| fault.map(|fault| (check, fault.severity)))
            .collect()
    }

    /// One service in a state, at a criticality, depending on nothing.
    fn service(id: &str, state: State, criticality: Criticality) -> Service {
        Service {
            id: id.to_owned(),
            name: id.to_owned(),
            profile: "media".to_owned(),
            state,
            criticality,
            exit: None,
            depends_on: Vec::new(),
        }
    }

    #[test]
    fn a_stack_with_nothing_wrong_reports_every_check_as_finding_nothing() {
        // Not an empty list: the store has to hear that these checks ran and were
        // fine, or a fault that has gone away would stand forever.
        let services = [service("sonarr", State::Healthy, Criticality::Core)];
        let looked = observed(&services, Egress::Behind);
        assert_eq!(looked.len(), 2, "the tunnel and the one service");
        assert!(looked.iter().all(|(_, fault)| fault.is_none()));
    }

    #[test]
    fn a_tunnel_that_does_not_apply_is_not_a_check_that_ran() {
        // Recording it as "found nothing" would clear a leak on a stack that has no
        // tunnel to leak from, which is a different claim entirely.
        let services = [service("sonarr", State::Healthy, Criticality::Core)];
        let checks: Vec<String> = observed(&services, Egress::NotApplicable)
            .into_iter()
            .map(|(check, _)| check)
            .collect();
        assert_eq!(checks, vec!["service.sonarr".to_owned()]);
    }

    #[test]
    fn a_host_managed_service_is_not_lemonfibers_to_report_on() {
        let services = [service("plex", State::HostManaged, Criticality::Core)];
        assert!(observed(&services, Egress::NotApplicable).is_empty());
    }

    #[test]
    fn a_leaking_tunnel_is_critical_however_healthy_the_containers_are() {
        let services = [service("qbittorrent", State::Healthy, Criticality::Core)];
        assert_eq!(
            wrong(&services, Egress::Leaking),
            vec![("vpn.egress".to_owned(), Severity::Critical)]
        );
    }

    #[test]
    fn a_tunnel_nobody_could_read_is_a_warning_rather_than_silence() {
        // Reporting "could not check" as safe is the failure the feature exists for.
        assert_eq!(
            wrong(&[], Egress::Unreadable),
            vec![("vpn.egress".to_owned(), Severity::Warning)]
        );
    }

    #[test]
    fn every_fault_carries_something_to_do_about_it() {
        // A fault an operator can do nothing about is a dead end. Enforced by the
        // constructor, checked here across every shape this module produces.
        let services = [
            service("a", State::Failed, Criticality::Core),
            service("b", State::CrashLooping, Criticality::Important),
            service("c", State::Unhealthy, Criticality::Optional),
        ];
        for egress in [Egress::Leaking, Egress::Unreadable] {
            for (check, fault) in observed(&services, egress) {
                let remedies = fault.map(|fault| fault.remedies).unwrap_or_default();
                assert!(!remedies.is_empty(), "{check}");
            }
        }
    }

    #[test]
    fn how_a_service_failed_changes_what_to_try_first() {
        // Restarting something that is already restarting itself achieves nothing.
        let cases = [
            (State::CrashLooping, "restarting itself"),
            (State::Unhealthy, "give"),
            (State::Failed, "start"),
        ];
        for (state, expected) in cases {
            let services = [service("sonarr", state, Criticality::Core)];
            let first = observed(&services, Egress::NotApplicable)
                .into_iter()
                .filter_map(|(_, fault)| fault)
                .flat_map(|fault| fault.remedies.into_iter().take(1))
                .collect::<Vec<String>>();
            assert!(
                first.iter().any(|remedy| remedy.contains(expected)),
                "{state:?}: {first:?}"
            );
        }
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
                wrong(&services, Egress::NotApplicable),
                vec![("service.x".to_owned(), expected)],
                "{criticality:?}"
            );
        }
    }

    #[test]
    fn only_the_states_that_want_attention_are_faults() {
        let services = [
            service("a", State::Healthy, Criticality::Core),
            service("b", State::Running, Criticality::Core),
            service("c", State::Starting, Criticality::Core),
            service("d", State::Stopped, Criticality::Core),
            service("e", State::Absent, Criticality::Core),
            service("g", State::Failed, Criticality::Core),
        ];
        assert_eq!(
            wrong(&services, Egress::NotApplicable),
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
            let summaries: Vec<String> = observed(&services, Egress::NotApplicable)
                .into_iter()
                .filter_map(|(_, fault)| fault.map(|fault| fault.summary))
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
        let summaries: Vec<String> = observed(&[failed], Egress::NotApplicable)
            .into_iter()
            .filter_map(|(_, fault)| fault.map(|fault| fault.summary))
            .collect();
        assert_eq!(
            summaries,
            vec!["sonarr stopped on its own (exit 137)".to_owned()]
        );
    }

    #[test]
    fn a_service_down_because_what_it_needs_is_down_names_the_one_underneath() {
        // One problem with the thing underneath, not two independent failures.
        let client = Service {
            depends_on: vec!["gluetun".to_owned()],
            ..service("qbittorrent", State::Failed, Criticality::Core)
        };
        let gateway = service("gluetun", State::Failed, Criticality::Critical);
        let causes: Vec<(String, Option<String>)> = observed(&[client, gateway], Egress::Behind)
            .into_iter()
            .filter_map(|(check, fault)| fault.map(|fault| (check, fault.caused_by)))
            .collect();
        assert_eq!(
            causes,
            vec![
                (
                    "service.qbittorrent".to_owned(),
                    Some("service.gluetun".to_owned())
                ),
                ("service.gluetun".to_owned(), None),
            ]
        );
    }

    #[test]
    fn a_service_down_while_what_it_needs_is_fine_stands_on_its_own() {
        let client = Service {
            depends_on: vec!["gluetun".to_owned()],
            ..service("qbittorrent", State::Failed, Criticality::Core)
        };
        let gateway = service("gluetun", State::Healthy, Criticality::Critical);
        let causes: Vec<Option<String>> = observed(&[client, gateway], Egress::Behind)
            .into_iter()
            .filter_map(|(_, fault)| fault.map(|fault| fault.caused_by))
            .collect();
        assert_eq!(causes, vec![None]);
    }

    #[test]
    fn an_unreadable_tunnel_names_the_gateway_that_is_why() {
        // The gateway being down is why the tunnel cannot be trusted, rather than a
        // second thing wrong beside it.
        let gateway = service("gluetun", State::Failed, Criticality::Critical);
        let causes: Vec<(String, Option<String>)> = observed(&[gateway], Egress::Unreadable)
            .into_iter()
            .filter_map(|(check, fault)| fault.map(|fault| (check, fault.caused_by)))
            .collect();
        assert_eq!(
            causes,
            vec![
                ("vpn.egress".to_owned(), Some("service.gluetun".to_owned())),
                ("service.gluetun".to_owned(), None),
            ]
        );
    }
}
