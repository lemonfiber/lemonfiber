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
        describes: format!("what {id} is for"),
        profile: "media".to_owned(),
        forms: Vec::new(),
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
fn what_a_failure_costs_is_said_differently_at_every_criticality() {
    // A sentence shared between two rungs would make the distinction
    // decorative, and an operator learns quickly which distinctions are.
    let said: Vec<String> = [
        Criticality::Critical,
        Criticality::Core,
        Criticality::Important,
        Criticality::Enhancing,
        Criticality::Optional,
    ]
    .into_iter()
    .filter_map(|criticality| {
        let services = [service("x", State::Failed, criticality)];
        observed(&services, Egress::NotApplicable)
            .into_iter()
            .find_map(|(_, fault)| fault.map(|fault| fault.meaning))
    })
    .collect();

    let mut distinct = said.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), 5, "{said:?}");
    assert!(said.iter().all(|meaning| meaning.contains('x')), "{said:?}");
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
