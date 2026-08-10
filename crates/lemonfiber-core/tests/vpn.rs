//! The VPN leak check, driven through its public surface.
//!
//! The check is built on an `#[async_trait]` engine, and async-trait code tested
//! from a `#[cfg(test)]` module in the same file produces phantom coverage from
//! being compiled twice. So — as with the engine adapter — it is exercised from
//! here instead, through the port, where the fake and the test bodies are not
//! themselves counted and the production code is covered once.

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_core::config::{PortForward, Protocols};
use lemonfiber_core::doctor::vpn::{
    VpnCheck, CLIENT_ISOLATED, LEAKING, NO_FORWARDED_PORT, VPN_CONTAINER_DOWN,
};
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
    /// What the container's forwarded-port status file reads, where it has one.
    forwarded_port: Option<&'static str>,
    /// A different answer for a second address service, where the test is about
    /// the sources contradicting each other.
    second_opinion: Option<&'static str>,
    exec_fails: bool,
}

impl Behavior {
    fn up(service: &'static str, address: Option<&'static str>) -> Self {
        Self {
            service,
            running: true,
            address,
            country: None,
            forwarded_port: None,
            second_opinion: None,
            exec_fails: false,
        }
    }
}

/// How the tunnel container answers the killswitch test, and what the client
/// sees while the tunnel is down.
#[derive(Debug, Clone, Copy)]
struct Link {
    /// The device the default route names, or nothing where there is no route.
    device: Option<&'static str>,
    /// Whether `ip link set` succeeds.
    movable: bool,
    /// Whether putting the device back up succeeds.
    restores: bool,
    /// What the client's address probe answers while the tunnel is down. `None`
    /// is traffic that stopped, which is the killswitch holding.
    leaks_as: Option<&'static str>,
}

impl Link {
    /// A tunnel that drops and comes back, and a client whose traffic stops with
    /// it — the stack the killswitch is supposed to produce.
    const fn holding() -> Self {
        Self {
            device: Some("tun0"),
            movable: true,
            restores: true,
            leaks_as: None,
        }
    }
}

/// An engine that answers exec and list from a fixed script.
struct Fake {
    reachable: bool,
    behaviors: Vec<Behavior>,
    /// How the killswitch probe is answered, where a test scripts it. Absent is
    /// an image with no `ip` in it.
    link: Option<Link>,
    /// Whether the tunnel device is down right now.
    dropped: std::sync::atomic::AtomicBool,
}

impl Fake {
    fn new(behaviors: Vec<Behavior>) -> Self {
        Self {
            reachable: true,
            behaviors,
            link: None,
            dropped: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// The same, answering the killswitch test as `link` says.
    fn linked(behaviors: Vec<Behavior>, link: Link) -> Self {
        Self {
            link: Some(link),
            ..Self::new(behaviors)
        }
    }

    /// Whether the tunnel device is currently down.
    fn is_dropped(&self) -> bool {
        self.dropped.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The tunnel container's answer to an `ip` command.
    fn answer_ip(&self, argv: &[String]) -> ExecOutput {
        let Some(link) = self.link else {
            // An image with no `ip` in it — a fork this test cannot be run against.
            return ExecOutput {
                status: Some(127),
                stdout: String::new(),
            };
        };
        if argv.get(1).is_some_and(|arg| arg == "route") {
            // A dropped device carries no default route, which is also how the
            // check confirms the restore took.
            let device = if self.is_dropped() { None } else { link.device };
            return ExecOutput {
                status: Some(0),
                stdout: device.map_or_else(String::new, |device| {
                    format!("default via 10.8.0.1 dev {device} proto static")
                }),
            };
        }
        let up = argv.last().is_some_and(|arg| arg == "up");
        if !link.movable || (up && !link.restores) {
            return ExecOutput {
                status: Some(2),
                stdout: String::new(),
            };
        }
        self.dropped
            .store(!up, std::sync::atomic::Ordering::Relaxed);
        ExecOutput {
            status: Some(0),
            stdout: String::new(),
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
        // The forwarded-port read: `cat` of the status file. A container with a
        // port answers it; one without makes `cat` exit non-zero with no output,
        // exactly as a missing file does.
        if argv.first().is_some_and(|arg| arg == "ip") {
            return Ok(self.answer_ip(argv));
        }
        if argv.first().is_some_and(|arg| arg == "cat") {
            return Ok(match behavior.forwarded_port {
                Some(port) => ExecOutput {
                    status: Some(0),
                    stdout: format!("{port}\n"),
                },
                None => ExecOutput {
                    status: Some(1),
                    stdout: String::new(),
                },
            });
        }
        let wants_country = argv.last().is_some_and(|arg| arg.ends_with("/country-iso"));
        if wants_country {
            return Ok(ExecOutput {
                status: Some(0),
                stdout: behavior.country.unwrap_or_default().to_owned(),
            });
        }
        // A second address service, where the test gave one a different answer —
        // which is what a cached or misconfigured echo looks like from here.
        let asked_second = argv
            .last()
            .is_some_and(|arg| arg.contains("second.example"));
        // While the tunnel is down the client answers with whatever the test says
        // still gets out — nothing at all, where the killswitch holds.
        let address = if self.is_dropped() && behavior.service != "gluetun" {
            self.link.and_then(|link| link.leaks_as)
        } else if asked_second {
            behavior.second_opinion.or(behavior.address)
        } else {
            behavior.address
        };
        match address {
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
/// port forwarding not configured, and the disruptive checks left off.
fn check(behaviors: Vec<Behavior>) -> VpnCheck {
    check_with(behaviors, PortForward::default())
}

/// A check as above, but with a chosen port-forward configuration, for the cases
/// that exercise the forwarded-port finding.
fn check_with(behaviors: Vec<Behavior>, port_forward: PortForward) -> VpnCheck {
    VpnCheck::new(
        Arc::new(Fake::new(behaviors)),
        "lemonfiber".to_owned(),
        &stack(),
        Protocols::both(),
        vec!["https://ifconfig.me".to_owned()],
        port_forward,
        false,
    )
}

/// A port-forward configuration: enabled, for the named provider.
fn forwarding(provider: &str) -> PortForward {
    PortForward {
        enabled: true,
        provider: Some(provider.to_owned()),
    }
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

/// The finding for a check, where the run produced one at all.
fn finding<'a>(findings: &'a [Finding], check: &str) -> Option<&'a Finding> {
    findings.iter().find(|finding| finding.check == check)
}

/// The problem carried by a failing finding, cloned so a test can read it after
/// the findings go out of scope.
fn failure(findings: &[Finding], check: &str) -> Option<Problem> {
    match verdict(findings, check) {
        Some(Verdict::Fail(problem)) => Some(problem.clone()),
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

/// The check over an engine, with the killswitch scripted.
fn checking(engine: Fake) -> VpnCheck {
    VpnCheck::new(
        Arc::new(engine),
        "lemonfiber".to_owned(),
        &stack(),
        Protocols::both(),
        vec!["https://ifconfig.me".to_owned()],
        PortForward::default(),
        true,
    )
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
            restores: false,
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
    let subject = VpnCheck::new(
        Arc::new(Fake::linked(contained(), Link::holding())),
        "lemonfiber".to_owned(),
        &stack(),
        Protocols::both(),
        vec!["https://ifconfig.me".to_owned()],
        PortForward::default(),
        false,
    );
    let findings = subject.run().await;
    assert!(unverified_reason(&findings, "vpn.killswitch")
        .is_some_and(|reason| reason.contains("interrupts transfers")));
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
        vec!["https://ifconfig.me".to_owned()],
        PortForward::default(),
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
        vec!["https://ifconfig.me".to_owned()],
        PortForward::default(),
        false,
    );
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
    let subject = VpnCheck::new(
        Arc::new(Fake::new(vec![
            Behavior::up("gluetun", Some("185.65.1.1")),
            Behavior::up("qbittorrent", Some("185.65.1.1")),
        ])),
        "lemonfiber".to_owned(),
        &stack(),
        Protocols::both(),
        Vec::new(),
        PortForward::default(),
        false,
    );
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
    let subject = VpnCheck::new(
        Arc::new(engine),
        "lemonfiber".to_owned(),
        &stack(),
        Protocols::both(),
        Vec::new(),
        PortForward::default(),
        false,
    );
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
    let subject = VpnCheck::new(
        Arc::new(Fake::new(vec![])),
        "lemonfiber".to_owned(),
        &empty(),
        Protocols::both(),
        vec!["https://ifconfig.me".to_owned()],
        PortForward::default(),
        false,
    );
    assert!(matches!(
        verdict(&subject.run().await, "vpn"),
        Some(Verdict::Skipped { .. })
    ));
}

/// A gluetun behaviour whose status file names a forwarded port.
fn gateway_with_port(port: &'static str) -> Behavior {
    let mut gluetun = Behavior::up("gluetun", Some("185.65.1.1"));
    gluetun.forwarded_port = Some(port);
    gluetun
}

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
        Protocols::both(),
        Vec::new(),
        forwarding("protonvpn"),
        false,
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
        Protocols::both(),
        vec!["https://ifconfig.me".to_owned()],
        forwarding("protonvpn"),
        false,
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
        Protocols::both(),
        vec!["https://ifconfig.me".to_owned()],
        PortForward::default(),
        false,
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
        Protocols::both(),
        vec![
            "https://first.example".to_owned(),
            "https://second.example".to_owned(),
        ],
        PortForward::default(),
        false,
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
