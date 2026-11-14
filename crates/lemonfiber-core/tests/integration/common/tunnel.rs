//! A container engine that answers as a tunnel and a client would.
//!
//! Shared by the tunnel's tests and the forwarded port's, because they drive the
//! same check against the same engine and differ only in what they script it to
//! say. Two copies would be two fakes drifting apart.

use std::sync::Arc;

use lemonfiber_core::config::{PortForward, Protocols};
use lemonfiber_core::doctor::vpn::VpnCheck;
use lemonfiber_core::doctor::vpn::{Asked, Grant};
use lemonfiber_core::doctor::{Finding, Verdict};
use lemonfiber_core::error::Problem;
pub use lemonfiber_fixtures::tunnelled::{Behavior, Fake, Link, Restore};
use lemonfiber_manifest::Manifest;

/// The stack this repository carries, whose torrent pair the check resolves.
const STACK: &str = lemonfiber_bundled::STACK;

/// An empty stack, for the case where no VPN pair can be resolved.
pub fn empty() -> Manifest {
    Manifest {
        schema_version: 1,
        stack_version: String::new(),
        min_cli_version: String::new(),
        include: Vec::new(),
        profiles: Vec::new(),
        forms: Vec::new(),
        services: Vec::new(),
        removed: Vec::new(),
        wirings: Vec::new(),
    }
}

/// The stack this repository carries.
pub fn stack() -> Manifest {
    Manifest::from_toml(STACK).unwrap_or_else(|_| empty())
}

/// What the VPN check is asked, with everything a test does not vary already answered.
///
/// The carried stack, both protocols, one IP echo, no port forwarding configured, no
/// client port, and the disruptive checks off. One builder rather than a helper per
/// axis, so a test that varies something new names it here instead of writing the
/// whole `Asked` out again.
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
            granted: Grant::Unreadable,
            port_forward: PortForward::default(),
            disruptive: false,
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
    /// Where that somewhere is not the granted port, the check reports the client off
    /// the forwarded port.
    #[must_use]
    pub const fn listening(mut self, port: u16) -> Self {
        self.asked.listening = Some(port);
        self
    }

    /// What the egress guard answered asked for its forwarded port, which the caller
    /// asks over `network.egress-guard`.
    #[must_use]
    pub const fn granted(mut self, granted: Grant) -> Self {
        self.asked.granted = granted;
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
