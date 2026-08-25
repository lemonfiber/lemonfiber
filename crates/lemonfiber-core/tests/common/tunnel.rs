//! A container engine that answers as a tunnel and a client would.
//!
//! Shared by the tunnel's tests and the forwarded port's, because they drive the
//! same check against the same engine and differ only in what they script it to
//! say. Two copies would be two fakes drifting apart.

#![allow(dead_code)]

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_core::config::{PortForward, Protocols};
use lemonfiber_core::doctor::vpn::Asked;
use lemonfiber_core::doctor::vpn::VpnCheck;
use lemonfiber_core::doctor::{Finding, Verdict};
use lemonfiber_core::error::Problem;
use lemonfiber_core::ports::docker::{
    Container, Engine, ExecOutput, Failure, Health, Lifecycle, LogLine, LogQuery, Stats,
};
use lemonfiber_manifest::Manifest;
use tokio::sync::mpsc::Receiver;

/// The stack this repository carries, whose torrent pair the check resolves.
const STACK: &str = include_str!("../../../../assets/media-stack/stack.toml");

/// How one container should answer the check.
#[derive(Clone)]
pub struct Behavior {
    /// The compose service this behaviour is for.
    pub service: &'static str,
    /// Whether the container is up.
    pub running: bool,
    /// The public address it reports, or nothing where it reports none.
    pub address: Option<&'static str>,
    /// The country that address resolves to, where the test cares.
    pub country: Option<&'static str>,
    /// What the container's forwarded-port status file reads, where it has one.
    pub forwarded_port: Option<&'static str>,
    /// A different answer for a second address service, where the test is about
    /// the sources contradicting each other.
    pub second_opinion: Option<&'static str>,
    /// Whether an exec into it fails outright, rather than answering.
    pub exec_fails: bool,
}

impl Behavior {
    /// A container that is running, reporting this address where it is asked for one.
    pub fn up(service: &'static str, address: Option<&'static str>) -> Self {
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

/// How the tunnel container answers being asked to put the device back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restore {
    /// The device comes back.
    Works,
    /// The command is refused.
    Fails,
    /// The command never answers at all.
    Silent,
}

/// How the tunnel container answers the killswitch test, and what the client
/// sees while the tunnel is down.
#[derive(Debug, Clone, Copy)]
pub struct Link {
    /// The device the default route names, or nothing where there is no route.
    pub device: Option<&'static str>,
    /// Whether `ip link set` succeeds.
    pub movable: bool,
    /// How putting the device back up is answered.
    pub restore: Restore,
    /// What the client's address probe answers while the tunnel is down. `None`
    /// is traffic that stopped, which is the killswitch holding.
    pub leaks_as: Option<&'static str>,
    /// Whether the client's probe never answers at all while the tunnel is down.
    ///
    /// A container whose network went away with the tunnel can leave an exec waiting
    /// with nothing to answer it, which is the shape of hang that outlasts a budget.
    pub probe_hangs: bool,
    /// How many seconds the route read takes, so a test can spend the budget before
    /// the tunnel is touched.
    pub route_takes: u64,
}

impl Link {
    /// A tunnel that drops and comes back, and a client whose traffic stops with
    /// it — the stack the killswitch is supposed to produce.
    pub const fn holding() -> Self {
        Self {
            device: Some("tun0"),
            movable: true,
            restore: Restore::Works,
            leaks_as: None,
            probe_hangs: false,
            route_takes: 0,
        }
    }
}

/// An engine that answers exec and list from a fixed script.
pub struct Fake {
    /// Whether the engine itself answers at all.
    pub reachable: bool,
    /// How each container answers, one entry per service.
    pub behaviors: Vec<Behavior>,
    /// How the killswitch probe is answered, where a test scripts it. Absent is
    /// an image with no `ip` in it.
    pub link: Option<Link>,
    /// Whether the tunnel device is down right now.
    pub dropped: std::sync::atomic::AtomicBool,
}

impl Fake {
    /// An engine answering with these behaviours, in the order they are given.
    pub fn new(behaviors: Vec<Behavior>) -> Self {
        Self {
            reachable: true,
            behaviors,
            link: None,
            dropped: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// The same, answering the killswitch test as `link` says.
    pub fn linked(behaviors: Vec<Behavior>, link: Link) -> Self {
        Self {
            link: Some(link),
            ..Self::new(behaviors)
        }
    }

    /// Whether the tunnel device is currently down.
    pub fn is_dropped(&self) -> bool {
        self.dropped.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// How long this `ip` command takes to answer, where the test slowed it down or
    /// stopped it answering at all.
    pub async fn dawdle(&self, argv: &[String]) {
        let Some(link) = self.link else { return };
        if argv.get(1).is_some_and(|arg| arg == "route") && link.route_takes > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(link.route_takes)).await;
        }
        if argv.last().is_some_and(|arg| arg == "up") && link.restore == Restore::Silent {
            std::future::pending::<()>().await;
        }
    }

    /// The tunnel container's answer to an `ip` command.
    pub fn answer_ip(&self, argv: &[String]) -> ExecOutput {
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
        if !link.movable || (up && link.restore != Restore::Works) {
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
            self.dawdle(argv).await;
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
        // A client cut off with the tunnel can leave the exec waiting rather than
        // answering, where the test asks for that.
        if self.is_dropped()
            && behavior.service != "gluetun"
            && self.link.is_some_and(|link| link.probe_hangs)
        {
            std::future::pending::<()>().await;
        }
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
pub fn empty() -> Manifest {
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
pub fn stack() -> Manifest {
    Manifest::from_toml(STACK).unwrap_or_else(|_| empty())
}

/// What the VPN check is asked, with everything a test does not vary already answered.
///
/// The carried stack, both protocols, one IP echo, no port forwarding configured, no
/// client port, and the disruptive checks off. Three helpers used to cover the three
/// axes anyone had needed, and every test needing a fourth wrote the whole `Asked` out
/// again — fourteen of them did, which is fourteen places a new field would land.
pub struct Asking {
    engine: Arc<Fake>,
    manifest: Manifest,
    asked: Asked,
}

/// A check over this engine, to vary by name from there.
pub fn asking(engine: Fake) -> Asking {
    asking_engine(Arc::new(engine))
}

/// The same, over an engine the test keeps a handle on — so it can ask afterwards
/// what state the check left the tunnel in.
pub fn asking_engine(engine: Arc<Fake>) -> Asking {
    Asking {
        engine,
        manifest: stack(),
        asked: Asked {
            protocols: Protocols::both(),
            echo: vec!["https://ifconfig.me".to_owned()],
            listening: None,
            port_forward: PortForward::default(),
            disruptive: false,
            client: None,
        },
    }
}

impl Asking {
    /// The stack it reads, where the test is about one shaped differently — a stack
    /// with no gateway to contain the client, or none at all.
    #[must_use]
    pub fn against(mut self, manifest: Manifest) -> Self {
        self.manifest = manifest;
        self
    }

    /// The addresses it asks what the world sees it as.
    #[must_use]
    pub fn echoing(mut self, echoes: &[&str]) -> Self {
        self.asked.echo = echoes.iter().map(|echo| (*echo).to_owned()).collect();
        self
    }

    /// Leak detection switched off, which is what an empty echo list means: the check
    /// has nowhere to ask, so it cannot compare.
    #[must_use]
    pub fn without_leak_detection(mut self) -> Self {
        self.asked.echo = Vec::new();
        self
    }

    /// A client the caller says is listening somewhere.
    ///
    /// Where that somewhere is not the granted port, this is the one fault in the
    /// category lemonfiber can put right itself — so it is what a repair is driven
    /// against.
    #[must_use]
    pub const fn listening(mut self, port: u16) -> Self {
        self.asked.listening = Some(port);
        self
    }

    /// A port-forward configuration, for the cases that exercise the forwarded-port
    /// finding.
    #[must_use]
    pub fn forwarding(mut self, port_forward: PortForward) -> Self {
        self.asked.port_forward = port_forward;
        self
    }

    /// The checks that change something, which are off unless a test is about them.
    #[must_use]
    pub const fn disruptive(mut self) -> Self {
        self.asked.disruptive = true;
        self
    }

    /// The protocols the stack is configured for.
    #[must_use]
    pub fn over(mut self, protocols: Protocols) -> Self {
        self.asked.protocols = protocols;
        self
    }

    /// The check itself.
    #[must_use]
    pub fn check(self) -> VpnCheck {
        VpnCheck::new(
            self.engine,
            "lemonfiber".to_owned(),
            &self.manifest,
            self.asked,
        )
    }
}

/// A check against the carried stack with nothing varied — the common case, spelled the
/// short way because most tests want it.
pub fn check(behaviors: Vec<Behavior>) -> VpnCheck {
    asking(Fake::new(behaviors)).check()
}

/// The same, with a chosen port-forward configuration.
pub fn check_with(behaviors: Vec<Behavior>, port_forward: PortForward) -> VpnCheck {
    asking(Fake::new(behaviors))
        .forwarding(port_forward)
        .check()
}

/// The same again, for a client the caller says is listening somewhere.
pub fn listening_on(
    behaviors: Vec<Behavior>,
    port_forward: PortForward,
    listening: Option<u16>,
) -> VpnCheck {
    let asking = asking(Fake::new(behaviors)).forwarding(port_forward);
    match listening {
        Some(port) => asking.listening(port),
        None => asking,
    }
    .check()
}

/// A port-forward configuration: enabled, for the named provider.
pub fn forwarding(provider: &str) -> PortForward {
    PortForward {
        enabled: true,
        provider: Some(provider.to_owned()),
    }
}

/// The verdict one named check reached, for a test asserting on exactly it.
pub fn verdict<'a>(findings: &'a [Finding], check: &str) -> Option<&'a Verdict> {
    findings
        .iter()
        .find(|finding| finding.check == check)
        .map(|finding| &finding.verdict)
}

/// The problem carried by a failing or warning finding.
pub fn problem<'a>(findings: &'a [Finding], check: &str) -> Option<&'a Problem> {
    match verdict(findings, check) {
        Some(Verdict::Fail(problem) | Verdict::Warn(problem)) => Some(problem),
        _ => None,
    }
}

/// The note carried by a passing finding.
pub fn pass_note(findings: &[Finding], check: &str) -> Option<String> {
    match verdict(findings, check) {
        Some(Verdict::Pass { note }) => note.clone(),
        _ => None,
    }
}

/// The finding for a check, where the run produced one at all.
pub fn finding<'a>(findings: &'a [Finding], check: &str) -> Option<&'a Finding> {
    findings.iter().find(|finding| finding.check == check)
}

/// The problem carried by a failing finding, cloned so a test can read it after
/// the findings go out of scope.
pub fn failure(findings: &[Finding], check: &str) -> Option<Problem> {
    match verdict(findings, check) {
        Some(Verdict::Fail(problem)) => Some(problem.clone()),
        _ => None,
    }
}

/// The reason carried by an unverified finding.
pub fn unverified_reason(findings: &[Finding], check: &str) -> Option<String> {
    match verdict(findings, check) {
        Some(Verdict::Unverified { reason, .. }) => Some(reason.clone()),
        _ => None,
    }
}

/// A gluetun behaviour whose status file names a forwarded port.
pub fn gateway_with_port(port: &'static str) -> Behavior {
    let mut gluetun = Behavior::up("gluetun", Some("185.65.1.1"));
    gluetun.forwarded_port = Some(port);
    gluetun
}
