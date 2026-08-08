//! Proving torrent traffic genuinely leaves through the tunnel.
//!
//! A running tunnel container proves nothing about the download client's
//! traffic, and checking an address in a browser tells you about the browser.
//! The only honest test is to ask each container, from inside its own network
//! namespace, for its public address and compare: if the client shares the
//! tunnel's namespace the answers must match, and if that sharing ever broke the
//! difference is immediately visible.
//!
//! lemonfiber runs the query inside the containers rather than reaching the
//! network itself, so the one third-party dependency belongs to the containers
//! and stays disableable.
//!
//! See `.docs/architecture/module-layout.md`.

mod findings;
mod killswitch;
mod leak;
mod port_forward;
mod probe;

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_manifest::{Manifest, Protocol};

use super::{Category, Check, Finding, Verdict};
use crate::config::{PortForward, Protocols};
use crate::error::Remedy;
use crate::ports::docker::{Container, Engine};

use findings::{assemble, finding, killswitch_findings, skipped, unreachable_engine};
use killswitch::{not_attempted, Held};
use leak::{labelled, Reach};
use port_forward::{no_port, port_forward_offline, Grant};
use probe::{exit_country, find, public_address, read_grant, running};

pub use killswitch::{KILLSWITCH_LEAKS, TUNNEL_NOT_RESTORED};
pub use leak::{CLIENT_ISOLATED, LEAKING, VPN_CONTAINER_DOWN};
pub use port_forward::NO_FORWARDED_PORT;

/// Gluetun's own record of the port its provider forwarded, written inside the
/// container when port forwarding is on. Read from there rather than from the
/// control server, so no API token is needed and a locked-down control server
/// cannot be mistaken for an absent port.
const FORWARDED_PORT_FILE: &str = "/tmp/gluetun/forwarded_port";

/// Why the port-forward check does not apply when the switch is off — shared so
/// the running and offline paths word it identically.
const NOT_ENABLED: &str = "port forwarding is not enabled, so there is no forwarded port to verify";

/// The capability a VPN gateway needs to route the traffic of the containers
/// sharing its network, and so the mark by which one is recognised — the unit is
/// capability, not the container's name.
const GATEWAY_CAPABILITY: &str = "NET_ADMIN";

/// The two containers the check compares: the tunnel and the client contained
/// by it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pair {
    /// The service that provides the tunnel.
    pub gateway: String,
    /// The download client whose traffic must traverse it.
    pub client: String,
}

/// The tunnel gateway and the download client contained by it, recognised by
/// capability rather than name so support is not tied to one provider or one
/// image.
///
/// The gateway is the torrent-profile service holding the routing capability;
/// the client is the torrent-profile service that depends on it.
pub(crate) fn resolve_pair(manifest: &Manifest) -> Option<Pair> {
    let torrent: Vec<&str> = manifest
        .profiles
        .iter()
        .filter(|profile| profile.protocol == Some(Protocol::Torrent))
        .map(|profile| profile.id.as_str())
        .collect();

    let gateway = manifest.services.iter().find(|service| {
        torrent.contains(&service.profile.as_str())
            && service
                .capabilities
                .iter()
                .any(|capability| capability == GATEWAY_CAPABILITY)
    })?;

    let client = manifest.services.iter().find(|service| {
        torrent.contains(&service.profile.as_str())
            && service.depends_on.iter().any(|on| on == &gateway.id)
    })?;

    Some(Pair {
        gateway: gateway.id.clone(),
        client: client.id.clone(),
    })
}

/// The VPN leak check: compare the client's egress against the tunnel's.
pub struct VpnCheck {
    engine: Arc<dyn Engine>,
    project: String,
    echo: Option<String>,
    target: Target,
    port_forward: PortForward,
    disruptive: bool,
}

/// Whether the check applies, and against what.
enum Target {
    /// The pair to compare.
    Pair(Pair),
    /// The check does not apply, and why.
    Skip(String),
}

impl VpnCheck {
    /// A VPN check for the given stack, or one that reports why it does not
    /// apply.
    ///
    /// The pair is resolved once, up front: without torrents configured there is
    /// nothing to contain, and a stack that declares no VPN-contained client has
    /// nothing to compare.
    ///
    /// `disruptive` records that the operator opted into the checks that disturb
    /// the running system — the killswitch test is one — so the killswitch
    /// finding can speak to what that will and will not yet do.
    #[must_use]
    pub fn new(
        engine: Arc<dyn Engine>,
        project: String,
        manifest: &Manifest,
        protocols: Protocols,
        echo: Option<String>,
        port_forward: PortForward,
        disruptive: bool,
    ) -> Self {
        let target = if protocols.torrent {
            resolve_pair(manifest).map_or_else(
                || Target::Skip("this stack declares no VPN-contained torrent client".to_owned()),
                Target::Pair,
            )
        } else {
            Target::Skip("torrent downloads are not configured".to_owned())
        };
        Self {
            engine,
            project,
            echo,
            target,
            port_forward,
            disruptive,
        }
    }

    /// Ask one container for its public address.
    async fn reach(&self, container: Option<&Container>, echo: &str) -> Reach {
        public_address(self.engine.as_ref(), container, echo).await
    }

    /// The tunnel's exit country, best effort — reported where the endpoint can
    /// supply it, omitted rather than guessed where it cannot.
    async fn country(&self, container: &Container, echo: &str) -> Option<String> {
        exit_country(self.engine.as_ref(), container, echo).await
    }

    /// The port-forward finding: whether a port was granted, where the operator
    /// asked for one.
    ///
    /// Independent of leak detection — the port is read from the gateway's own
    /// status file, not from an IP-echo comparison — so it is still established
    /// where the operator has switched leak detection off. Where the switch is
    /// off the exec is skipped entirely: there is nothing to read.
    async fn port_forward_finding(&self, gateway: Option<&Container>) -> Finding {
        let verdict = if self.port_forward.enabled {
            match self.granted_port(gateway).await {
                Grant::Port(port) => Verdict::Pass {
                    note: Some(format!("forwarded port {port}")),
                },
                Grant::Absent => no_port(self.port_forward.provider.as_deref()),
                Grant::Unreadable => Verdict::Unverified {
                    reason: "the VPN container did not return a forwarded port, which the tunnel \
                             being down and the engine being unreachable both produce"
                        .to_owned(),
                    remedy: Remedy::new("Confirm the tunnel is up, then run this again"),
                },
            }
        } else {
            Verdict::Skipped {
                reason: NOT_ENABLED.to_owned(),
            }
        };
        finding("vpn.port-forward", "forwarded port", verdict)
    }

    /// Prove the killswitch: take the tunnel away, ask the client whether it can
    /// still reach the internet, and put the tunnel back.
    ///
    /// Only where the operator asked for the disruptive checks, and only where
    /// there is something to prove — a client that is not reaching the internet
    /// right now would answer "blocked" whatever the killswitch does, and calling
    /// that a pass would be the comfortable falsehood again in a new place.
    ///
    /// Whatever the probe finds, the tunnel goes back up. That restoration is
    /// attempted unconditionally and then confirmed; a tunnel this check took away
    /// and could not return is reported as the fault it is rather than left for
    /// the operator to discover.
    async fn killswitch_held(
        &self,
        gateway: Option<&Container>,
        client: Option<&Container>,
        echo: &str,
        client_now: &Reach,
    ) -> Held {
        if !self.disruptive {
            return Held::NotAttempted {
                reason: killswitch::NOT_ASKED_FOR.to_owned(),
            };
        }
        let Some(gateway) = running(gateway) else {
            return not_attempted(true, "the tunnel container is not running");
        };
        let Some(client) = running(client) else {
            return not_attempted(true, "the download client is not running");
        };
        // Nothing to prove against: a client already unable to reach the internet
        // answers "blocked" whether or not the killswitch had anything to do with
        // it, and reading that as a pass would prove nothing at all.
        if !matches!(client_now, Reach::Address(_)) {
            return not_attempted(
                true,
                "the download client is not reaching the internet right now, so stopping it \
                 would prove nothing",
            );
        }
        let Some(device) = self.tunnel_device(gateway).await else {
            return not_attempted(
                true,
                "the tunnel container carries no default route to drop",
            );
        };
        if !self.set_link(gateway, &device, false).await {
            return not_attempted(true, "the tunnel device could not be taken down");
        }

        // From here the stack is disturbed, so every path restores before it
        // returns — the probe's own answer is held until that has happened.
        //
        // The CLIENT is asked, not the gateway. The gateway's own traffic is not
        // the question; whether the container behind it still reaches the world
        // with the tunnel gone is the entire test.
        let held = killswitch::held_from(&self.reach(Some(client), echo).await);
        let restored = self.set_link(gateway, &device, true).await
            && self.tunnel_device(gateway).await.is_some();
        if restored {
            held
        } else {
            Held::NotRestored
        }
    }

    /// Which device carries the gateway's default route — the tunnel, whatever it
    /// is called.
    async fn tunnel_device(&self, gateway: &Container) -> Option<String> {
        let output = self
            .engine
            .exec(&gateway.id, &killswitch::read_route())
            .await
            .ok()?;
        killswitch::tunnel_device(&output.stdout)
    }

    /// Take the gateway's tunnel device down, or put it back. Whether the engine
    /// carried the command out at all.
    async fn set_link(&self, gateway: &Container, device: &str, up: bool) -> bool {
        self.engine
            .exec(&gateway.id, &killswitch::set_link(device, up))
            .await
            .is_ok_and(|output| output.status == Some(0))
    }

    /// Read the granted port from the gateway's own status file.
    ///
    /// A container that is not running, or an engine that cannot be reached, makes
    /// the answer unknown rather than absent. A file that reads as no port — empty,
    /// missing, or the zero the release path writes — is a port that was not
    /// granted, which for an enabled provider is the failure this check exists for.
    async fn granted_port(&self, gateway: Option<&Container>) -> Grant {
        read_grant(self.engine.as_ref(), gateway).await
    }
}

/// The dashboard's VPN telemetry, or why it is not available — a small reading the
/// panel maps directly, kept apart from the leak check's `Verdict`s so the panel
/// need not read one.
pub(crate) enum VpnReading {
    /// This stack has no VPN-contained torrent client, so the panel does not apply.
    NotApplicable,
    /// The tunnel could not be read; the reason the panel shows.
    Unavailable(String),
    /// The tunnel answered.
    Ready {
        /// The tunnel's exit address.
        exit_ip: String,
        /// Its country, where the endpoint supplied one.
        country: Option<String>,
        /// The provider's forwarded port, where forwarding is on and granted.
        forwarded_port: Option<u16>,
        /// Whether the download client's egress matches the tunnel's.
        egress_matches: bool,
    },
}

/// Read what the VPN panel shows: the tunnel's exit address and country, the
/// forwarded port, and whether the download client's egress matches the tunnel's
/// — the one thing that proves traffic leaves through it. The same containers and
/// exec-reads the leak check uses, shaped for a panel rather than a verdict.
///
/// Every read costs a round-trip into a container, so on the refresh loop this
/// wants caching — the tunnel's address changes rarely. That caching arrives with
/// the loop; until then the panel reads afresh each time.
pub(crate) async fn read_vpn(
    engine: &dyn Engine,
    project: &str,
    manifest: &Manifest,
    protocols: Protocols,
    echo: Option<&str>,
    port_forward_enabled: bool,
) -> VpnReading {
    if !protocols.torrent {
        return VpnReading::NotApplicable;
    }
    let Some(pair) = resolve_pair(manifest) else {
        return VpnReading::NotApplicable;
    };
    let Ok(containers) = engine.list(project).await else {
        return VpnReading::Unavailable(
            "the container engine could not be reached, so the VPN could not be asked".to_owned(),
        );
    };
    let gateway_container = find(&containers, &pair.gateway);

    // The forwarded port comes from the gateway's status file, independent of the
    // IP-echo, so it is read even where leak detection is off — but only where the
    // operator asked for forwarding at all.
    let forwarded_port = if port_forward_enabled {
        match read_grant(engine, gateway_container).await {
            Grant::Port(port) => Some(port),
            Grant::Absent | Grant::Unreadable => None,
        }
    } else {
        None
    };

    let Some(echo) = echo else {
        return VpnReading::Unavailable(
            "leak detection is switched off, so the tunnel's egress cannot be read".to_owned(),
        );
    };

    let gateway = public_address(engine, gateway_container, echo).await;
    // The country is the gateway's, asked only where the gateway both answered and
    // is a container we can ask — the same case that yields an address. The
    // catch-all absorbs every other combination (including the address-without-a-
    // container that cannot occur), so there is no unreachable arm to leave
    // uncovered.
    let country = match (&gateway, gateway_container) {
        (Reach::Address(_), Some(container)) => exit_country(engine, container, echo).await,
        _ => None,
    };
    match gateway {
        Reach::Address(exit_ip) => {
            let client = public_address(engine, find(&containers, &pair.client), echo).await;
            let egress_matches = matches!(&client, Reach::Address(ip) if *ip == exit_ip);
            VpnReading::Ready {
                exit_ip,
                country,
                forwarded_port,
                egress_matches,
            }
        }
        Reach::Down => VpnReading::Unavailable("the VPN tunnel is not running".to_owned()),
        Reach::Blocked => {
            VpnReading::Unavailable("the VPN tunnel did not return an exit address".to_owned())
        }
        Reach::Unknown => {
            VpnReading::Unavailable("the VPN container could not be reached".to_owned())
        }
    }
}

/// The findings for an active check, given both containers' answers.

#[async_trait]
impl Check for VpnCheck {
    fn category(&self) -> Category {
        Category::Vpn
    }

    async fn run(&self) -> Vec<Finding> {
        let pair = match &self.target {
            Target::Skip(reason) => return vec![skipped(reason.clone())],
            Target::Pair(pair) => pair,
        };

        // The engine being unreachable is a reason the checks could not run,
        // never a report that the stack is safe. Leak detection being switched off
        // is an opt-out that still holds here: the operator asked not to be told
        // about egress, so it stays skipped rather than becoming an unverified
        // engine finding — while port forwarding, which they did ask for, reports.
        let Ok(containers) = self.engine.list(&self.project).await else {
            return match self.echo {
                Some(_) => unreachable_engine(pair, &self.port_forward, self.disruptive),
                None => vec![
                    skipped("leak detection is switched off".to_owned()),
                    port_forward_offline(&self.port_forward),
                ],
            };
        };
        // Nothing is running, so the whole VPN check collapses to one line rather
        // than repeating "cannot check, nothing is up" for each finding — the
        // port-forward finding included, since its port lives in a container that
        // is not there either.
        if containers.is_empty() {
            return vec![skipped("the stack is not running".to_owned())];
        }

        let gateway_container = find(&containers, &pair.gateway);

        // Port forwarding is read from the gateway's own status file rather than
        // from the IP-echo comparison, so it is established even where the operator
        // has switched leak detection off.
        let port_forward = self.port_forward_finding(gateway_container).await;

        let Some(echo) = self.echo.as_deref() else {
            return vec![
                skipped("leak detection is switched off".to_owned()),
                port_forward,
            ];
        };

        let client_container = find(&containers, &pair.client);

        let gateway = self.reach(gateway_container, echo).await;
        let client = self.reach(client_container, echo).await;

        // The exit country only means anything where the tunnel answered, and is
        // one extra request, so it is asked for only then. Awaited into a plain
        // value first: a block whose last statement is an await leaves its own
        // closing brace unmarked by coverage.
        let country = match (&gateway, gateway_container) {
            (Reach::Address(_), Some(container)) => self.country(container, echo).await,
            _ => None,
        };
        let note = match &gateway {
            Reach::Address(ip) => Some(labelled(ip, country)),
            _ => None,
        };

        let held = self
            .killswitch_held(gateway_container, client_container, echo, &client)
            .await;
        let mut findings = assemble(pair, &gateway, &client, note, killswitch_findings(&held));
        findings.push(port_forward);
        findings
    }
}
