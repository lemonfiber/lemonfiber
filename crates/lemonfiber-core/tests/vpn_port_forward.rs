//! The forwarded port: whether one was granted, and whether the client is on it.
//!
//! Its own crate rather than more of the tunnel's, because the questions differ.
//! A tunnel that is up says nothing about a port, and a port that is forwarded
//! says nothing about whether anything outside can reach it.

mod common;

use common::tunnel::*;
use std::sync::Arc;

use lemonfiber_core::config::{PortForward, Protocols};
use lemonfiber_core::doctor::vpn::{Asked, VpnCheck, NO_FORWARDED_PORT};
use lemonfiber_core::doctor::{Check, Verdict};
use lemonfiber_core::error::Severity;

#[tokio::test]
async fn a_granted_port_is_a_verified_forward() {
    let findings = check_with(
        vec![
            gateway_with_port("51413"),
            Behavior::up("qbittorrent", Some("185.65.1.1")),
        ],
        forwarding("protonvpn"),
    )
    .run()
    .await;
    assert!(
        pass_note(&findings, "vpn.port-forward").is_some_and(|note| note.contains("51413")),
        "a granted port passes and states the port it saw"
    );
}

#[tokio::test]
async fn no_port_on_protonvpn_names_the_nat_pmp_trap_first() {
    // Tunnel up, but no port granted: ProtonVPN's cause — forwarding not enabled
    // when the config was generated — is the one an operator cannot guess, so it
    // is named as the cause rather than left to them.
    let findings = check_with(
        vec![
            Behavior::up("gluetun", Some("185.65.1.1")),
            Behavior::up("qbittorrent", Some("185.65.1.1")),
        ],
        forwarding("protonvpn"),
    )
    .run()
    .await;
    let problem = problem(&findings, "vpn.port-forward");
    assert_eq!(problem.map(|problem| problem.code), Some(NO_FORWARDED_PORT));
    // Degraded, not broken: nothing is leaking, peers simply cannot connect in.
    assert_eq!(
        problem.map(|problem| problem.severity),
        Some(Severity::Warning)
    );
    assert!(
        problem
            .is_some_and(|problem| problem.meaning.contains("NAT-PMP")
                && problem.meaning.contains("ProtonVPN")),
        "ProtonVPN's NAT-PMP-at-generation trap must be named: {problem:?}"
    );
}

#[tokio::test]
async fn no_port_on_another_forwarding_provider_is_degraded_without_speculation() {
    // A provider that forwards ports but has no lemonfiber-specific trap: still a
    // degraded finding, but it must not borrow ProtonVPN's cause.
    let findings = check_with(
        vec![
            Behavior::up("gluetun", Some("185.65.1.1")),
            Behavior::up("qbittorrent", Some("185.65.1.1")),
        ],
        forwarding("private internet access"),
    )
    .run()
    .await;
    let problem = problem(&findings, "vpn.port-forward");
    assert_eq!(problem.map(|problem| problem.code), Some(NO_FORWARDED_PORT));
    assert!(
        problem.is_some_and(|problem| !problem.meaning.contains("ProtonVPN")),
        "a different provider must not be told ProtonVPN's cause: {problem:?}"
    );
}

#[tokio::test]
async fn no_port_on_an_unknown_provider_is_unverified_not_blamed() {
    // lemonfiber has no port-forwarding knowledge of this provider, so it cannot
    // say why a port is missing without guessing — and guessing is what the
    // whole subsystem exists to avoid. Unverified, never a fabricated failure.
    let findings = check_with(
        vec![
            Behavior::up("gluetun", Some("185.65.1.1")),
            Behavior::up("qbittorrent", Some("185.65.1.1")),
        ],
        forwarding("some-obscure-vpn"),
    )
    .run()
    .await;
    assert!(matches!(
        verdict(&findings, "vpn.port-forward"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_released_port_reads_as_no_port() {
    // The release path writes a literal 0; that is a port taken back, not a port
    // granted, so it must be read as absent.
    let findings = check_with(
        vec![
            gateway_with_port("0"),
            Behavior::up("qbittorrent", Some("185.65.1.1")),
        ],
        forwarding("protonvpn"),
    )
    .run()
    .await;
    assert_eq!(
        problem(&findings, "vpn.port-forward").map(|problem| problem.code),
        Some(NO_FORWARDED_PORT)
    );
}

#[tokio::test]
async fn unusable_status_file_contents_read_as_no_port() {
    // A reachable status file that names no usable port — non-numeric, out of the
    // u16 a port fits in, or only whitespace — is a port not granted, not a failed
    // read. For a provider known to forward, that is the degraded finding.
    for contents in ["not-a-port", "70000", "   "] {
        let findings = check_with(
            vec![
                gateway_with_port(contents),
                Behavior::up("qbittorrent", Some("185.65.1.1")),
            ],
            forwarding("protonvpn"),
        )
        .run()
        .await;
        assert_eq!(
            problem(&findings, "vpn.port-forward").map(|problem| problem.code),
            Some(NO_FORWARDED_PORT),
            "{contents:?} names no usable port, so it is a port not granted"
        );
    }
}

#[tokio::test]
async fn port_forwarding_not_enabled_does_not_apply() {
    // Even with a running tunnel, an operator who did not ask for port forwarding
    // gets a not-applicable finding, never a fault — and the status file is not
    // even read.
    let findings = check(vec![
        gateway_with_port("51413"),
        Behavior::up("qbittorrent", Some("185.65.1.1")),
    ])
    .run()
    .await;
    assert!(matches!(
        verdict(&findings, "vpn.port-forward"),
        Some(Verdict::Skipped { .. })
    ));
}

#[tokio::test]
async fn a_missing_gateway_leaves_the_forward_unverified() {
    // The stack is up but the tunnel container is not among it, so there is no
    // status file to read: the port is unknown, not absent.
    let findings = check_with(
        vec![Behavior::up("qbittorrent", Some("81.2.3.4"))],
        forwarding("protonvpn"),
    )
    .run()
    .await;
    assert!(matches!(
        verdict(&findings, "vpn.port-forward"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_gateway_that_cannot_be_read_leaves_the_forward_unverified() {
    // The tunnel container is up, but the command to read its status file could
    // not be run at all — unknown, so unverified rather than a port not granted.
    let mut gluetun = gateway_with_port("51413");
    gluetun.exec_fails = true;
    let findings = check_with(
        vec![gluetun, Behavior::up("qbittorrent", None)],
        forwarding("protonvpn"),
    )
    .run()
    .await;
    assert!(matches!(
        verdict(&findings, "vpn.port-forward"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_down_gateway_leaves_the_forward_unverified() {
    // The port lives in the tunnel container; with it down the port is unknown,
    // not absent, so the honest answer is unverified rather than a failure.
    let mut gluetun = Behavior::up("gluetun", None);
    gluetun.running = false;
    let findings = check_with(vec![gluetun], forwarding("protonvpn"))
        .run()
        .await;
    assert!(matches!(
        verdict(&findings, "vpn.port-forward"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn port_forwarding_is_checked_even_with_leak_detection_off() {
    // The port is read from the container's own file, not from an IP-echo
    // comparison, so switching leak detection off does not blind this check.
    let subject = VpnCheck::new(
        Arc::new(Fake::new(vec![gateway_with_port("51413")])),
        "lemonfiber".to_owned(),
        &stack(),
        Asked {
            protocols: Protocols::both(),
            echo: Vec::new(),
            listening: None,
            port_forward: forwarding("protonvpn"),
            disruptive: false,
            client: None,
        },
    );
    let findings = subject.run().await;
    assert!(
        pass_note(&findings, "vpn.port-forward").is_some_and(|note| note.contains("51413")),
        "port forwarding is still verified with leak detection off"
    );
}

#[tokio::test]
async fn an_unreachable_engine_leaves_an_enabled_forward_unverified() {
    let mut engine = Fake::new(vec![]);
    engine.reachable = false;
    let subject = VpnCheck::new(
        Arc::new(engine),
        "lemonfiber".to_owned(),
        &stack(),
        Asked {
            protocols: Protocols::both(),
            echo: vec!["https://ifconfig.me".to_owned()],
            listening: None,
            port_forward: forwarding("protonvpn"),
            disruptive: false,
            client: None,
        },
    );
    let findings = subject.run().await;
    assert!(matches!(
        verdict(&findings, "vpn.port-forward"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_gateway_with_no_client_does_not_apply() {
    // A stack with the tunnel but no client depending on it resolves no pair, so
    // there is nothing whose egress to compare.
    let mut lone_gateway = stack();
    lone_gateway.services.retain(|service| {
        !service
            .depends_on
            .iter()
            .any(|dependency| dependency == "gluetun")
    });
    let subject = VpnCheck::new(
        Arc::new(Fake::new(vec![])),
        "lemonfiber".to_owned(),
        &lone_gateway,
        Asked {
            protocols: Protocols::both(),
            echo: vec!["https://ifconfig.me".to_owned()],
            listening: None,
            port_forward: PortForward::default(),
            disruptive: false,
            client: None,
        },
    );
    assert!(matches!(
        verdict(&subject.run().await, "vpn"),
        Some(Verdict::Skipped { .. })
    ));
}

/// A pair whose two address services disagree about the gateway's egress.
fn contradicted() -> Fake {
    Fake::new(vec![
        Behavior {
            second_opinion: Some("198.51.100.9"),
            ..Behavior::up("gluetun", Some("185.65.1.1"))
        },
        Behavior::up("qbittorrent", Some("185.65.1.1")),
    ])
}

#[tokio::test]
async fn address_services_that_contradict_each_other_are_reported_rather_than_resolved() {
    // The whole leak verdict is a comparison against one number. A source that is
    // cached, misconfigured or simply wrong returns a plausible address, and a
    // check that picked a winner would say `pass` while traffic left in the clear.
    let subject = VpnCheck::new(
        Arc::new(contradicted()),
        "lemonfiber".to_owned(),
        &stack(),
        Asked {
            protocols: Protocols::both(),
            echo: vec![
                "https://first.example".to_owned(),
                "https://second.example".to_owned(),
            ],
            listening: None,
            port_forward: PortForward::default(),
            disruptive: false,
            client: None,
        },
    );
    let findings = subject.run().await;

    let disagreement = findings
        .iter()
        .find(|finding| finding.check == "vpn.egress-sources");
    let reason = disagreement.and_then(|finding| match &finding.verdict {
        Verdict::Unverified { reason, .. } => Some(reason.clone()),
        _ => None,
    });
    let reason = reason.unwrap_or_default();
    assert!(reason.contains("185.65.1.1"), "{reason}");
    assert!(reason.contains("198.51.100.9"), "{reason}");

    // And nothing anywhere claims the tunnel was proven, because it was not.
    assert!(
        !findings
            .iter()
            .any(|finding| matches!(finding.verdict, Verdict::Pass { .. })),
        "nothing passes on an address nobody agreed"
    );
}

#[tokio::test]
async fn a_client_listening_off_the_forwarded_port_is_reported_rather_than_corrected() {
    // A diagnosis is only looking: the operator asked what is wrong, not for
    // anything to be changed. Starting the stack applies the same fix, because by
    // then they have asked for an action.
    let mut gateway = Behavior::up("gluetun", Some("185.65.1.1"));
    gateway.forwarded_port = Some("51999");
    let subject = VpnCheck::new(
        Arc::new(Fake::new(vec![
            gateway,
            Behavior::up("qbittorrent", Some("185.65.1.1")),
        ])),
        "lemonfiber".to_owned(),
        &stack(),
        Asked {
            protocols: Protocols::both(),
            echo: vec!["https://ifconfig.me".to_owned()],
            listening: Some(51413),
            port_forward: forwarding("proton"),
            disruptive: false,
            client: None,
        },
    );
    let findings = subject.run().await;
    let mismatch = findings
        .iter()
        .find(|finding| finding.check == "vpn.port-forward-client");
    let summary = mismatch.and_then(|finding| match &finding.verdict {
        Verdict::Warn(problem) => Some(problem.summary.clone()),
        _ => None,
    });
    let summary = summary.unwrap_or_default();
    assert!(summary.contains("51999"), "{summary}");
    assert!(summary.contains("51413"), "{summary}");
}

#[tokio::test]
async fn a_client_already_on_the_forwarded_port_is_not_reported() {
    let mut gateway = Behavior::up("gluetun", Some("185.65.1.1"));
    gateway.forwarded_port = Some("51413");
    let subject = VpnCheck::new(
        Arc::new(Fake::new(vec![
            gateway,
            Behavior::up("qbittorrent", Some("185.65.1.1")),
        ])),
        "lemonfiber".to_owned(),
        &stack(),
        Asked {
            protocols: Protocols::both(),
            echo: vec!["https://ifconfig.me".to_owned()],
            listening: Some(51413),
            port_forward: forwarding("proton"),
            disruptive: false,
            client: None,
        },
    );
    assert!(subject
        .run()
        .await
        .iter()
        .all(|finding| finding.check != "vpn.port-forward-client"));
}

#[tokio::test]
async fn the_granted_port_is_read_from_the_gateways_own_status_file() {
    // The same file the check and the panel read, so nothing has a second opinion
    // about what the provider granted.
    let mut gateway = Behavior::up("gluetun", Some("185.65.1.1"));
    gateway.forwarded_port = Some("51413");
    let engine = Fake::new(vec![
        gateway,
        Behavior::up("qbittorrent", Some("185.65.1.1")),
    ]);
    assert_eq!(
        lemonfiber_core::doctor::vpn::granted_port(&engine, "lemonfiber", &stack(), true).await,
        Some(51413)
    );
}

#[tokio::test]
async fn no_port_is_read_where_forwarding_was_never_asked_for() {
    // Nothing is granted because nothing was requested; reading the file anyway
    // would report a stale port from a previous configuration.
    let mut gateway = Behavior::up("gluetun", Some("185.65.1.1"));
    gateway.forwarded_port = Some("51413");
    let engine = Fake::new(vec![gateway]);
    assert_eq!(
        lemonfiber_core::doctor::vpn::granted_port(&engine, "lemonfiber", &stack(), false).await,
        None
    );
}

#[tokio::test]
async fn a_gateway_with_no_granted_port_reads_as_none_rather_than_a_failure() {
    let engine = Fake::new(vec![Behavior::up("gluetun", Some("185.65.1.1"))]);
    assert_eq!(
        lemonfiber_core::doctor::vpn::granted_port(&engine, "lemonfiber", &stack(), true).await,
        None
    );
}

#[tokio::test]
async fn a_torrent_client_with_nothing_containing_it_is_warned_not_skipped() {
    // The arrangement this whole category exists to catch, and the one it used to
    // pass over: with no gateway the pair does not resolve, and reporting that as
    // "does not apply" reads as though the check looked and found nothing to look
    // at — while what it found is torrent traffic leaving under the operator's own
    // address.
    let mut uncontained = stack();
    uncontained
        .services
        .retain(|service| service.id != "gluetun");
    let subject = VpnCheck::new(
        Arc::new(Fake::new(vec![])),
        "lemonfiber".to_owned(),
        &uncontained,
        Asked {
            protocols: Protocols::both(),
            echo: vec!["https://ifconfig.me".to_owned()],
            listening: None,
            port_forward: PortForward::default(),
            disruptive: false,
            client: None,
        },
    );
    let findings = subject.run().await;
    assert!(
        matches!(
            verdict(&findings, "vpn.unprotected"),
            Some(Verdict::Warn(_))
        ),
        "a choice with a cost, never a failure and never a skip"
    );
    assert!(
        verdict(&findings, "vpn").is_none(),
        "and not also reported as not applying"
    );
}
