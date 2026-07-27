//! The VPN leak check, driven through its public surface.
//!
//! The check is built on an `#[async_trait]` engine, and async-trait code tested
//! from a `#[cfg(test)]` module in the same file produces phantom coverage from
//! being compiled twice. So — as with the engine adapter — it is exercised from
//! here instead, through the port, where the fake and the test bodies are not
//! themselves counted and the production code is covered once.

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_core::config::Protocols;
use lemonfiber_core::doctor::vpn::{VpnCheck, CLIENT_ISOLATED, LEAKING, VPN_CONTAINER_DOWN};
use lemonfiber_core::doctor::{Category, Check, Finding, Verdict};
use lemonfiber_core::error::{Problem, Severity};
use lemonfiber_core::ports::docker::{
    Container, Engine, ExecOutput, Failure, Health, Lifecycle, LogLine, LogQuery, Stats,
};
use lemonfiber_manifest::Manifest;
use tokio::sync::mpsc::Receiver;

/// The stack this repository carries, whose torrent pair the check resolves.
const STACK: &str = include_str!("../../../assets/media-stack/stack.toml");

/// How one container should answer the check.
#[derive(Clone)]
struct Behavior {
    service: &'static str,
    running: bool,
    address: Option<&'static str>,
    country: Option<&'static str>,
    exec_fails: bool,
}

impl Behavior {
    fn up(service: &'static str, address: Option<&'static str>) -> Self {
        Self {
            service,
            running: true,
            address,
            country: None,
            exec_fails: false,
        }
    }
}

/// An engine that answers exec and list from a fixed script.
struct Fake {
    reachable: bool,
    behaviors: Vec<Behavior>,
}

impl Fake {
    fn new(behaviors: Vec<Behavior>) -> Self {
        Self {
            reachable: true,
            behaviors,
        }
    }
}

#[async_trait]
impl Engine for Fake {
    async fn list(&self, _project: &str) -> Result<Vec<Container>, Failure> {
        if !self.reachable {
            return Err(Failure::Unreachable {
                reason: "no daemon".to_owned(),
            });
        }
        Ok(self
            .behaviors
            .iter()
            .map(|behavior| Container {
                id: format!("id-{}", behavior.service),
                project: "lemonfiber".to_owned(),
                service: behavior.service.to_owned(),
                lifecycle: if behavior.running {
                    Lifecycle::Running
                } else {
                    Lifecycle::Exited
                },
                health: Health::None,
                exit: None,
            })
            .collect())
    }

    async fn exec(&self, container: &str, argv: &[String]) -> Result<ExecOutput, Failure> {
        let behavior = self
            .behaviors
            .iter()
            .find(|behavior| format!("id-{}", behavior.service) == container)
            .ok_or_else(|| Failure::NoSuchContainer {
                name: container.to_owned(),
            })?;
        if behavior.exec_fails {
            return Err(Failure::NoSuchContainer {
                name: container.to_owned(),
            });
        }
        let wants_country = argv.last().is_some_and(|arg| arg.ends_with("/country-iso"));
        if wants_country {
            return Ok(ExecOutput {
                status: Some(0),
                stdout: behavior.country.unwrap_or_default().to_owned(),
            });
        }
        match behavior.address {
            Some(address) => Ok(ExecOutput {
                status: Some(0),
                stdout: format!("{address}\n"),
            }),
            None => Ok(ExecOutput {
                status: Some(1),
                stdout: String::new(),
            }),
        }
    }

    async fn stats(&self, _project: &str) -> Result<Receiver<(String, Stats)>, Failure> {
        let (_sender, receiver) = tokio::sync::mpsc::channel(1);
        Ok(receiver)
    }

    async fn logs(
        &self,
        _project: &str,
        _services: &[String],
        _query: LogQuery,
    ) -> Result<Receiver<LogLine>, Failure> {
        let (_sender, receiver) = tokio::sync::mpsc::channel(1);
        Ok(receiver)
    }
}

/// An empty stack, for the case where no VPN pair can be resolved.
fn empty() -> Manifest {
    Manifest {
        schema_version: 1,
        stack_version: String::new(),
        min_cli_version: String::new(),
        profiles: Vec::new(),
        forms: Vec::new(),
        services: Vec::new(),
    }
}

/// The stack this repository carries.
fn stack() -> Manifest {
    Manifest::from_toml(STACK).unwrap_or_else(|_| empty())
}

/// A check against the carried stack, with both protocols and leak detection on,
/// and the disruptive checks left off.
fn check(behaviors: Vec<Behavior>) -> VpnCheck {
    VpnCheck::new(
        Arc::new(Fake::new(behaviors)),
        "lemonfiber".to_owned(),
        &stack(),
        Protocols::both(),
        Some("https://ifconfig.me".to_owned()),
        false,
    )
}

fn verdict<'a>(findings: &'a [Finding], check: &str) -> Option<&'a Verdict> {
    findings
        .iter()
        .find(|finding| finding.check == check)
        .map(|finding| &finding.verdict)
}

/// The problem carried by a failing or warning finding.
fn problem<'a>(findings: &'a [Finding], check: &str) -> Option<&'a Problem> {
    match verdict(findings, check) {
        Some(Verdict::Fail(problem) | Verdict::Warn(problem)) => Some(problem),
        _ => None,
    }
}

/// The note carried by a passing finding.
fn pass_note(findings: &[Finding], check: &str) -> Option<String> {
    match verdict(findings, check) {
        Some(Verdict::Pass { note }) => note.clone(),
        _ => None,
    }
}

/// The reason carried by an unverified finding.
fn unverified_reason(findings: &[Finding], check: &str) -> Option<String> {
    match verdict(findings, check) {
        Some(Verdict::Unverified { reason, .. }) => Some(reason.clone()),
        _ => None,
    }
}

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

#[tokio::test]
async fn opting_into_the_disruptive_check_says_it_is_not_yet_built() {
    // The killswitch is unverified either way; with --disruptive it must not
    // point back at the flag the operator already gave.
    let subject = VpnCheck::new(
        Arc::new(Fake::new(vec![
            Behavior::up("gluetun", Some("185.65.1.1")),
            Behavior::up("qbittorrent", Some("185.65.1.1")),
        ])),
        "lemonfiber".to_owned(),
        &stack(),
        Protocols::both(),
        Some("https://ifconfig.me".to_owned()),
        true,
    );
    let findings = subject.run().await;
    assert!(unverified_reason(&findings, "vpn.killswitch")
        .is_some_and(|reason| reason.contains("not yet built")));
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
    let subject = VpnCheck::new(
        Arc::new(engine),
        "lemonfiber".to_owned(),
        &stack(),
        Protocols::both(),
        Some("https://ifconfig.me".to_owned()),
        false,
    );
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
    let subject = VpnCheck::new(
        Arc::new(Fake::new(vec![])),
        "lemonfiber".to_owned(),
        &stack(),
        Protocols::none(),
        Some("https://ifconfig.me".to_owned()),
        false,
    );
    assert!(matches!(
        verdict(&subject.run().await, "vpn"),
        Some(Verdict::Skipped { .. })
    ));
}

#[tokio::test]
async fn leak_detection_switched_off_does_not_apply() {
    let subject = VpnCheck::new(
        Arc::new(Fake::new(vec![])),
        "lemonfiber".to_owned(),
        &stack(),
        Protocols::both(),
        None,
        false,
    );
    assert!(matches!(
        verdict(&subject.run().await, "vpn"),
        Some(Verdict::Skipped { .. })
    ));
}

#[tokio::test]
async fn a_stack_with_no_gateway_does_not_apply() {
    let subject = VpnCheck::new(
        Arc::new(Fake::new(vec![])),
        "lemonfiber".to_owned(),
        &empty(),
        Protocols::both(),
        Some("https://ifconfig.me".to_owned()),
        false,
    );
    assert!(matches!(
        verdict(&subject.run().await, "vpn"),
        Some(Verdict::Skipped { .. })
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
        Protocols::both(),
        Some("https://ifconfig.me".to_owned()),
        false,
    );
    assert!(matches!(
        verdict(&subject.run().await, "vpn"),
        Some(Verdict::Skipped { .. })
    ));
}
