//! The VPN leak check, driven through its public surface.
//!
//! The check is built on an `#[async_trait]` engine, and async-trait code tested
//! from a `#[cfg(test)]` module in the same file produces phantom coverage from
//! being compiled twice. So — as with the engine adapter — it is exercised from
//! here instead, through the port, where the fake and the test bodies are not
//! themselves counted and the production code is covered once.

mod common;

use std::sync::Arc;

use common::tunnel::*;

use lemonfiber_core::config::Protocols;
use lemonfiber_core::doctor::vpn::{VpnCheck, CLIENT_ISOLATED, LEAKING, VPN_CONTAINER_DOWN};
use lemonfiber_core::doctor::{examine, Category, Check, Narrowing, Verdict};
use lemonfiber_core::error::Severity;

#[tokio::test]
async fn matching_addresses_are_a_verified_tunnel() {
    let mut gluetun = Behavior::up("gluetun", Some("185.65.1.1"));
    gluetun.country = Some("nl");
    let subject = check(vec![
        gluetun,
        Behavior::up("qbittorrent", Some("185.65.1.1")),
    ]);
    assert_eq!(subject.category(), Category::Vpn);
    let findings = subject.run().await;

    let note = pass_note(&findings, "vpn.tunnel");
    assert!(
        note.as_deref()
            .is_some_and(|note| note.contains("185.65.1.1") && note.contains("NL")),
        "tunnel note should carry the address and exit country: {note:?}"
    );
    assert!(matches!(
        verdict(&findings, "vpn.egress-match"),
        Some(Verdict::Pass { .. })
    ));
    assert!(matches!(
        verdict(&findings, "vpn.killswitch"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_matching_ipv6_address_is_also_verified() {
    // Exercises the address check against a colon-bearing address.
    let findings = check(vec![
        Behavior::up("gluetun", Some("2a02:1234:abcd::1")),
        Behavior::up("qbittorrent", Some("2a02:1234:abcd::1")),
    ])
    .run()
    .await;
    assert!(matches!(
        verdict(&findings, "vpn.egress-match"),
        Some(Verdict::Pass { .. })
    ));
}

/// The check over an engine, with the killswitch scripted.
fn checking(engine: Fake) -> VpnCheck {
    asking(engine).disruptive().check()
}

/// A tunnel and a client that both answer from the same address — the healthy
/// pair every killswitch test starts from.
fn contained() -> Vec<Behavior> {
    vec![
        Behavior::up("gluetun", Some("185.65.1.1")),
        Behavior::up("qbittorrent", Some("185.65.1.1")),
    ]
}

#[tokio::test]
async fn a_killswitch_that_holds_is_proven_by_taking_the_tunnel_away() {
    // The whole point: not inferred from a running container, but established by
    // dropping the tunnel and watching the client's traffic stop with it.
    let findings = checking(Fake::linked(contained(), Link::holding()))
        .run()
        .await;
    assert!(
        matches!(
            verdict(&findings, "vpn.killswitch"),
            Some(Verdict::Pass { .. })
        ),
        "{:?}",
        verdict(&findings, "vpn.killswitch")
    );
    assert!(
        finding(&findings, "vpn.tunnel-restored").is_none(),
        "a tunnel that came back needs no second finding"
    );
}

#[tokio::test]
async fn traffic_that_survives_the_tunnel_is_the_leak_this_check_exists_for() {
    // A stack whose killswitch is a comfortable assumption: the tunnel goes away
    // and the torrents carry on, in the open, from the operator's own address.
    let findings = checking(Fake::linked(
        contained(),
        Link {
            leaks_as: Some("203.0.113.9"),
            ..Link::holding()
        },
    ))
    .run()
    .await;
    let problem = failure(&findings, "vpn.killswitch");
    assert_eq!(problem.as_ref().map(|p| p.severity), Some(Severity::Error));
    assert!(
        problem
            .and_then(|p| p.detail)
            .is_some_and(|detail| detail.contains("203.0.113.9")),
        "the address the world saw is named"
    );
}

#[tokio::test]
async fn a_tunnel_the_test_could_not_put_back_is_reported_twice_over() {
    // This check breaks something on purpose. Failing to restore it is not a
    // footnote on the killswitch verdict — it is its own emergency, and folding
    // the two into one finding would report whichever was written last.
    let findings = checking(Fake::linked(
        contained(),
        Link {
            restore: Restore::Fails,
            ..Link::holding()
        },
    ))
    .run()
    .await;
    assert!(failure(&findings, "vpn.killswitch").is_some());
    let restored = failure(&findings, "vpn.tunnel-restored");
    assert!(
        restored.is_some_and(|problem| problem.summary.contains("did not come back")),
        "the operator is told their tunnel is gone"
    );
}

#[tokio::test]
async fn a_tunnel_that_cannot_be_dropped_disturbs_nothing_and_proves_nothing() {
    // An image with no `ip`, a route with no device, a link that will not move:
    // each leaves the stack exactly as it was, and none of them is a pass.
    let unmovable = [
        Link {
            device: None,
            ..Link::holding()
        },
        Link {
            movable: false,
            ..Link::holding()
        },
    ];
    for link in unmovable {
        let findings = checking(Fake::linked(contained(), link)).run().await;
        assert!(
            unverified_reason(&findings, "vpn.killswitch").is_some(),
            "{link:?} should prove nothing"
        );
    }
    // And no `ip` at all.
    let findings = checking(Fake::new(contained())).run().await;
    assert!(unverified_reason(&findings, "vpn.killswitch").is_some());
}

#[tokio::test]
async fn a_pair_that_is_not_both_running_is_not_tested_against() {
    // Nothing to drop, or nothing to ask afterwards. Either way the stack is left
    // exactly as it was found.
    let down_gateway = vec![
        Behavior {
            running: false,
            ..Behavior::up("gluetun", Some("185.65.1.1"))
        },
        Behavior::up("qbittorrent", Some("185.65.1.1")),
    ];
    let findings = checking(Fake::linked(down_gateway, Link::holding()))
        .run()
        .await;
    assert!(unverified_reason(&findings, "vpn.killswitch")
        .is_some_and(|reason| reason.contains("tunnel container is not running")));

    let down_client = vec![
        Behavior::up("gluetun", Some("185.65.1.1")),
        Behavior {
            running: false,
            ..Behavior::up("qbittorrent", Some("185.65.1.1"))
        },
    ];
    let findings = checking(Fake::linked(down_client, Link::holding()))
        .run()
        .await;
    assert!(unverified_reason(&findings, "vpn.killswitch")
        .is_some_and(|reason| reason.contains("download client is not running")));
}

#[tokio::test]
async fn a_tunnel_container_that_will_not_take_a_command_is_left_alone() {
    // The route cannot even be read, so there is nothing to drop and nothing is.
    let findings = checking(Fake::linked(
        vec![
            Behavior {
                exec_fails: true,
                ..Behavior::up("gluetun", Some("185.65.1.1"))
            },
            Behavior::up("qbittorrent", Some("185.65.1.1")),
        ],
        Link::holding(),
    ))
    .run()
    .await;
    assert!(unverified_reason(&findings, "vpn.killswitch")
        .is_some_and(|reason| reason.contains("no default route")));
}

#[tokio::test]
async fn a_client_already_off_the_internet_proves_nothing_about_the_killswitch() {
    // It would answer "blocked" whatever the killswitch does, so reading that as
    // a pass would be the comfortable falsehood again in a new place.
    let findings = checking(Fake::linked(
        vec![
            Behavior::up("gluetun", Some("185.65.1.1")),
            Behavior::up("qbittorrent", None),
        ],
        Link::holding(),
    ))
    .run()
    .await;
    assert!(unverified_reason(&findings, "vpn.killswitch")
        .is_some_and(|reason| reason.contains("prove nothing")));
}

#[tokio::test]
async fn without_the_flag_nothing_is_touched_and_the_operator_is_told_how_to_ask() {
    let subject = asking(Fake::linked(contained(), Link::holding())).check();
    let findings = subject.run().await;
    assert!(unverified_reason(&findings, "vpn.killswitch")
        .is_some_and(|reason| reason.contains("interrupts transfers")));
}

/// The suite as it is actually run, over an engine the test keeps hold of, so what
/// state the check left the tunnel in can be asked afterwards.
async fn suite_over(engine: &Arc<Fake>) -> lemonfiber_core::model::DoctorReport {
    let checks: Vec<Box<dyn Check>> = vec![Box::new(
        asking_engine(Arc::clone(engine)).disruptive().check(),
    )];
    examine(&checks, &Narrowing::Suite).await
}

/// A probe that outlasts the budget still leaves the tunnel up.
///
/// The suite abandons a check at its budget by dropping the future it is awaiting, and
/// a dropped future runs no further code. Nothing between taking the tunnel away and
/// putting it back may therefore be allowed to reach that moment: a client cut off with
/// the tunnel is exactly the thing likeliest to stop answering, and the restore behind
/// it is the one part of this check that must always happen.
#[tokio::test(start_paused = true)]
async fn a_probe_that_outlasts_the_budget_leaves_the_tunnel_up_all_the_same() {
    let engine = Arc::new(Fake::linked(
        contained(),
        Link {
            probe_hangs: true,
            ..Link::holding()
        },
    ));

    let report = suite_over(&engine).await;

    assert!(
        !engine.is_dropped(),
        "the tunnel was taken away and never put back: {report:?}"
    );
    assert!(
        unverified_reason(&report.findings, "vpn.killswitch")
            .is_some_and(|reason| reason.contains("could not be asked")),
        "a client that never answered proves nothing: {report:?}"
    );
}

/// Putting it back is bounded too, so a restore that never answers is reported as the
/// emergency it is rather than holding the run open until the suite gives up on it.
#[tokio::test(start_paused = true)]
async fn a_restore_that_never_answers_is_reported_rather_than_waited_on() {
    let engine = Arc::new(Fake::linked(
        contained(),
        Link {
            restore: Restore::Silent,
            ..Link::holding()
        },
    ));

    let report = suite_over(&engine).await;

    let restored = failure(&report.findings, "vpn.tunnel-restored");
    assert!(
        restored.is_some_and(|problem| problem.summary.contains("did not come back")),
        "the operator is told the tunnel is gone rather than left waiting: {report:?}"
    );
}

/// A budget already spent by the reads before it is not enough to drop the tunnel and
/// be sure of putting it back, so the tunnel is not dropped at all.
#[tokio::test(start_paused = true)]
async fn a_budget_already_spent_never_takes_the_tunnel_away() {
    let engine = Arc::new(Fake::linked(
        contained(),
        Link {
            route_takes: 20,
            ..Link::holding()
        },
    ));

    let report = suite_over(&engine).await;

    assert!(
        !engine.is_dropped(),
        "nothing should have been disturbed: {report:?}"
    );
    assert!(
        unverified_reason(&report.findings, "vpn.killswitch")
            .is_some_and(|reason| reason.contains("too little of this check's time")),
        "{report:?}"
    );
}

#[tokio::test]
async fn a_differing_address_is_a_critical_leak() {
    let findings = check(vec![
        Behavior::up("gluetun", Some("185.65.1.1")),
        Behavior::up("qbittorrent", Some("81.2.3.4")),
    ])
    .run()
    .await;
    assert_eq!(
        problem(&findings, "vpn.egress-match").map(|problem| problem.code),
        Some(LEAKING)
    );
    assert_eq!(
        problem(&findings, "vpn.egress-match").map(|problem| problem.severity),
        Some(Severity::Critical)
    );
    // The tunnel still passed, so the note carries the address without a country
    // the endpoint was not asked for.
    assert!(matches!(
        verdict(&findings, "vpn.tunnel"),
        Some(Verdict::Pass { note: Some(_) })
    ));
}

#[tokio::test]
async fn a_client_online_while_the_tunnel_is_not_is_a_leak() {
    let findings = check(vec![
        Behavior::up("gluetun", None),
        Behavior::up("qbittorrent", Some("81.2.3.4")),
    ])
    .run()
    .await;
    assert_eq!(
        problem(&findings, "vpn.egress-match").map(|problem| problem.code),
        Some(LEAKING)
    );
    assert!(matches!(
        verdict(&findings, "vpn.tunnel"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_tunnel_that_cannot_be_asked_is_unverified_not_a_leak() {
    // The client returned an address, but the tunnel's own could not be read at
    // all. Unknown is not proven-absent, so egress cannot be compared and no
    // leak is claimed — the honest answer is unverified, never a critical alarm.
    let mut gluetun = Behavior::up("gluetun", None);
    gluetun.exec_fails = true;
    let findings = check(vec![gluetun, Behavior::up("qbittorrent", Some("81.2.3.4"))])
        .run()
        .await;
    assert!(
        unverified_reason(&findings, "vpn.egress-match")
            .is_some_and(|reason| reason.contains("could not be compared")),
        "an unaskable tunnel makes egress unverified, not a leak"
    );
}

#[tokio::test]
async fn a_contained_client_with_no_connectivity_is_a_warning() {
    let findings = check(vec![
        Behavior::up("gluetun", Some("185.65.1.1")),
        Behavior::up("qbittorrent", None),
    ])
    .run()
    .await;
    assert_eq!(
        problem(&findings, "vpn.egress-match").map(|problem| problem.code),
        Some(CLIENT_ISOLATED)
    );
}

#[tokio::test]
async fn both_unreachable_cannot_confirm_safety() {
    let findings = check(vec![
        Behavior::up("gluetun", None),
        Behavior::up("qbittorrent", None),
    ])
    .run()
    .await;
    assert!(matches!(
        verdict(&findings, "vpn.tunnel"),
        Some(Verdict::Unverified { .. })
    ));
    assert!(matches!(
        verdict(&findings, "vpn.egress-match"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_stopped_vpn_container_is_a_definite_fault() {
    let mut gluetun = Behavior::up("gluetun", None);
    gluetun.running = false;
    let mut qbit = Behavior::up("qbittorrent", None);
    qbit.running = false;
    let findings = check(vec![gluetun, qbit]).run().await;
    assert_eq!(
        problem(&findings, "vpn.tunnel").map(|problem| problem.code),
        Some(VPN_CONTAINER_DOWN)
    );
    assert!(matches!(
        verdict(&findings, "vpn.egress-match"),
        Some(Verdict::Skipped { .. })
    ));
}

#[tokio::test]
async fn a_missing_vpn_container_is_a_definite_fault() {
    // The client is up and reaching the internet with no tunnel container at
    // all — the tunnel is definitely down, and the client is leaking.
    let findings = check(vec![Behavior::up("qbittorrent", Some("81.2.3.4"))])
        .run()
        .await;
    assert_eq!(
        problem(&findings, "vpn.tunnel").map(|problem| problem.code),
        Some(VPN_CONTAINER_DOWN)
    );
    assert_eq!(
        problem(&findings, "vpn.egress-match").map(|problem| problem.code),
        Some(LEAKING)
    );
}

#[tokio::test]
async fn a_client_that_cannot_be_asked_is_unverified() {
    let mut qbit = Behavior::up("qbittorrent", Some("185.65.1.1"));
    qbit.exec_fails = true;
    let findings = check(vec![Behavior::up("gluetun", Some("185.65.1.1")), qbit])
        .run()
        .await;
    assert!(matches!(
        verdict(&findings, "vpn.egress-match"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_gateway_that_cannot_be_asked_is_unverified() {
    let mut gluetun = Behavior::up("gluetun", Some("185.65.1.1"));
    gluetun.exec_fails = true;
    let findings = check(vec![gluetun, Behavior::up("qbittorrent", None)])
        .run()
        .await;
    assert!(matches!(
        verdict(&findings, "vpn.tunnel"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_garbage_response_is_not_taken_for_an_address() {
    // A status-zero body that is not an address is no clean answer, so it is
    // treated as no address rather than compared.
    let findings = check(vec![
        Behavior::up("gluetun", Some("not-an-address")),
        Behavior::up("qbittorrent", Some("not-an-address")),
    ])
    .run()
    .await;
    assert!(matches!(
        verdict(&findings, "vpn.tunnel"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn an_unreachable_engine_leaves_the_checks_unverified() {
    let mut engine = Fake::new(vec![]);
    engine.reachable = false;
    let subject = asking(engine).check();
    let findings = subject.run().await;
    assert!(matches!(
        verdict(&findings, "vpn.tunnel"),
        Some(Verdict::Unverified { .. })
    ));
    assert!(matches!(
        verdict(&findings, "vpn.egress-match"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_stopped_stack_skips_rather_than_fails() {
    let findings = check(vec![]).run().await;
    assert!(matches!(
        verdict(&findings, "vpn"),
        Some(Verdict::Skipped { .. })
    ));
}

#[tokio::test]
async fn no_torrents_configured_does_not_apply() {
    let subject = asking(Fake::new(vec![])).over(Protocols::none()).check();
    assert!(matches!(
        verdict(&subject.run().await, "vpn"),
        Some(Verdict::Skipped { .. })
    ));
}

#[tokio::test]
async fn leak_detection_switched_off_does_not_apply() {
    // A running stack, so this exercises the echo-off branch itself rather than
    // the stack-not-running one: with leak detection off the egress comparison is
    // skipped rather than run.
    let subject = asking(Fake::new(vec![
        Behavior::up("gluetun", Some("185.65.1.1")),
        Behavior::up("qbittorrent", Some("185.65.1.1")),
    ]))
    .without_leak_detection()
    .check();
    assert!(matches!(
        verdict(&subject.run().await, "vpn"),
        Some(Verdict::Skipped { .. })
    ));
}

#[tokio::test]
async fn leak_detection_off_holds_even_when_the_engine_is_down() {
    // The opt-out is the operator's, and it stands whether or not the engine can
    // be reached: egress stays skipped, never an engine-unreachable unverified.
    let mut engine = Fake::new(vec![]);
    engine.reachable = false;
    let subject = asking(engine).without_leak_detection().check();
    let findings = subject.run().await;
    // The egress group is skipped, not reported unverified against a down engine.
    assert!(matches!(
        verdict(&findings, "vpn"),
        Some(Verdict::Skipped { .. })
    ));
    assert!(
        verdict(&findings, "vpn.egress-match").is_none(),
        "leak detection is off, so no egress finding is produced at all"
    );
}

#[tokio::test]
async fn a_stack_with_no_gateway_does_not_apply() {
    let subject = asking(Fake::new(vec![])).against(empty()).check();
    assert!(matches!(
        verdict(&subject.run().await, "vpn"),
        Some(Verdict::Skipped { .. })
    ));
}
