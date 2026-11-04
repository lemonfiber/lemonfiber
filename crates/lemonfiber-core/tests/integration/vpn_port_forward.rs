//! The forwarded port: whether one was granted, and whether the client is on it.
//!
//! Its own crate rather than more of the tunnel's, because the questions differ.
//! A tunnel that is up says nothing about a port, and a port that is forwarded
//! says nothing about whether anything outside can reach it.

use common::tunnel::*;

use crate::common;
use lemonfiber_core::doctor::vpn::{Grant, VpnCheck};
use lemonfiber_core::doctor::{Check, Verdict};
use lemonfiber_core::error::codes::vpn::NO_FORWARDED_PORT;
use lemonfiber_core::error::Severity;

/// The tunnel and its client up, port forwarding asked of `provider`, and the egress
/// guard having answered `granted`.
fn granting(provider: &str, granted: Grant) -> VpnCheck {
    asking(Fake::new(vec![
        Behavior::up("gluetun", Some("185.65.1.1")),
        Behavior::up("qbittorrent", Some("185.65.1.1")),
    ]))
    .forwarding(forwarding(provider))
    .granted(granted)
    .check()
}

#[tokio::test]
async fn a_granted_port_is_a_verified_forward() {
    let findings = granting("protonvpn", Grant::Port(51413)).run().await;
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
    let findings = granting("protonvpn", Grant::Absent).run().await;
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
    let findings = granting("private internet access", Grant::Absent)
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
    let findings = granting("some-obscure-vpn", Grant::Absent).run().await;
    assert!(matches!(
        verdict(&findings, "vpn.port-forward"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_released_port_or_none_answered_reads_as_no_port() {
    // A guard whose provider took the port back answers zero, or no port at all; both
    // are a port not granted rather than an unknown one.
    for answered in [Some(0), None] {
        let findings = granting("protonvpn", Grant::of(Some(answered))).run().await;
        assert_eq!(
            problem(&findings, "vpn.port-forward").map(|problem| problem.code),
            Some(NO_FORWARDED_PORT),
            "{answered:?}"
        );
    }
}

#[tokio::test]
async fn port_forwarding_not_enabled_does_not_apply() {
    // Even with a running tunnel and a port granted, an operator who did not ask for
    // port forwarding gets a not-applicable finding, never a fault.
    let findings = asking(Fake::new(vec![
        Behavior::up("gluetun", Some("185.65.1.1")),
        Behavior::up("qbittorrent", Some("185.65.1.1")),
    ]))
    .granted(Grant::Port(51413))
    .check()
    .run()
    .await;
    assert!(matches!(
        verdict(&findings, "vpn.port-forward"),
        Some(Verdict::Skipped { .. })
    ));
}

#[tokio::test]
async fn a_missing_gateway_leaves_the_forward_unverified() {
    // The stack is up but the tunnel container is not among it, so there is no guard
    // to answer: the port is unknown, not absent.
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
async fn a_guard_that_did_not_answer_leaves_the_forward_unverified() {
    // The tunnel container is up, but its guard could not be asked — unknown, so
    // unverified rather than a port not granted.
    let findings = granting("protonvpn", Grant::Unreadable).run().await;
    assert!(matches!(
        verdict(&findings, "vpn.port-forward"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_down_gateway_leaves_the_forward_unverified() {
    // With the tunnel down its guard answers nothing, so the port is unknown, not
    // absent, and the honest answer is unverified rather than a failure.
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
    // The port is what the guard answered, not an IP-echo comparison, so switching
    // leak detection off does not blind this check.
    let subject = asking(Fake::new(vec![Behavior::up("gluetun", Some("185.65.1.1"))]))
        .without_leak_detection()
        .forwarding(forwarding("protonvpn"))
        .granted(Grant::Port(51413))
        .check();
    let findings = subject.run().await;
    assert!(
        pass_note(&findings, "vpn.port-forward").is_some_and(|note| note.contains("51413")),
        "port forwarding is still verified with leak detection off"
    );
}

#[tokio::test]
async fn a_port_the_guard_answered_is_verified_where_the_engine_cannot_be_reached() {
    let mut engine = Fake::new(vec![]);
    engine.reachable = false;
    let findings = asking(engine)
        .forwarding(forwarding("protonvpn"))
        .granted(Grant::Port(51413))
        .check()
        .run()
        .await;
    assert!(
        pass_note(&findings, "vpn.port-forward").is_some_and(|note| note.contains("51413")),
        "the port the guard answered does not wait on the engine"
    );
}

#[tokio::test]
async fn an_unreachable_engine_leaves_an_enabled_forward_unverified() {
    let mut engine = Fake::new(vec![]);
    engine.reachable = false;
    let subject = asking(engine).forwarding(forwarding("protonvpn")).check();
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
    let subject = asking(Fake::new(vec![])).against(lone_gateway).check();
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
    let subject = asking(contradicted())
        .echoing(&["https://first.example", "https://second.example"])
        .check();
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

    // Nor does anything claim the opposite. A disagreement is not read as no
    // connectivity: a gateway whose sources contradict each other while the client
    // answers normally is no leak, and reporting one would raise an alarm about the
    // client on the strength of two strangers not agreeing.
    assert!(
        !findings
            .iter()
            .any(|finding| matches!(finding.verdict, Verdict::Fail(_))),
        "a disagreement among sources is not evidence of a leak either: {:?}",
        findings
            .iter()
            .filter(|finding| matches!(finding.verdict, Verdict::Fail(_)))
            .map(|finding| finding.check.clone())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn a_client_listening_off_the_forwarded_port_is_reported_rather_than_corrected() {
    // A diagnosis is only looking: the operator asked what is wrong, not for
    // anything to be changed. Starting the stack applies the same fix, because by
    // then they have asked for an action.
    let subject = asking(Fake::new(vec![
        Behavior::up("gluetun", Some("185.65.1.1")),
        Behavior::up("qbittorrent", Some("185.65.1.1")),
    ]))
    .listening(51413)
    .forwarding(forwarding("proton"))
    .granted(Grant::Port(51999))
    .check();
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
    let subject = asking(Fake::new(vec![
        Behavior::up("gluetun", Some("185.65.1.1")),
        Behavior::up("qbittorrent", Some("185.65.1.1")),
    ]))
    .listening(51413)
    .forwarding(forwarding("proton"))
    .granted(Grant::Port(51413))
    .check();
    assert!(subject
        .run()
        .await
        .iter()
        .all(|finding| finding.check != "vpn.port-forward-client"));
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
    let subject = asking(Fake::new(vec![])).against(uncontained).check();
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

/// A tunnel nothing runs inside contains no client, so the client is reported as
/// unprotected, as it is when there is no tunnel at all.
#[tokio::test]
async fn a_torrent_client_outside_the_tunnel_s_network_is_warned_not_skipped() {
    let mut outside = stack();
    for service in &mut outside.services {
        if service.id == "qbittorrent" {
            service.network = None;
        }
    }
    let findings = asking(Fake::new(vec![]))
        .against(outside)
        .check()
        .run()
        .await;
    assert!(matches!(
        verdict(&findings, "vpn.unprotected"),
        Some(Verdict::Warn(_))
    ));
    assert!(verdict(&findings, "vpn").is_none());
}
