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

mod echo;
mod findings;
mod forwarding;
mod killswitch;
mod leak;
mod mender;
mod pair;
mod port_forward;
mod probe;
mod reading;

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_manifest::Manifest;

use super::{Category, Check, Finding, Verdict};
use crate::config::{PortForward, Protocols};
use crate::ports::docker::{Container, Engine};
use crate::qbittorrent::Qbittorrent;

pub use echo::Seen;
use findings::{
    assemble, disagreeing, killswitch_findings, port_mismatch, skipped, unprotected,
    unreachable_engine,
};
pub use forwarding::{Answer, Forwarding};
use leak::{labelled, Reach};
use pair::torrent_client;
pub(crate) use pair::{resolve_pair, Pair};
pub use port_forward::granted_port;
use port_forward::{port_forward_offline, Grant};
use probe::{addresses, exit_country, find, public_address, read_grant};

pub use killswitch::{KILLSWITCH_LEAKS, TUNNEL_NOT_RESTORED};
pub use leak::{CLIENT_ISOLATED, LEAKING, VPN_CONTAINER_DOWN};
pub use port_forward::NO_FORWARDED_PORT;
pub(crate) use reading::{read_vpn, VpnReading};

/// Gluetun's own record of the port its provider forwarded, written inside the
/// container when port forwarding is on. Read from there rather than from the
/// control server, so no API token is needed and a locked-down control server
/// cannot be mistaken for an absent port.
const FORWARDED_PORT_FILE: &str = "/tmp/gluetun/forwarded_port";

/// Why the port-forward check does not apply when the switch is off — shared so
/// the running and offline paths word it identically.
const NOT_ENABLED: &str = "port forwarding is not enabled, so there is no forwarded port to verify";

/// The VPN leak check: compare the client's egress against the tunnel's.
pub struct VpnCheck {
    engine: Arc<dyn Engine>,
    project: String,
    echo: Vec<String>,
    /// What the download client says it is listening on, where it could be asked.
    /// Read by the caller: this check speaks to containers, not to services.
    listening: Option<u16>,
    target: Target,
    port_forward: PortForward,
    disruptive: bool,
    /// What this check can put right, where a client could be authenticated to.
    ///
    /// Held rather than built on demand: the correction needs credentials, and reading
    /// those is the caller's business — this check speaks to containers.
    mender: Option<mender::PortMender>,
}

/// What the operator asked for, and what was read on their behalf, gathered so
/// the check takes one value rather than a row of loose arguments whose order is
/// the only thing keeping them apart.
pub struct Asked {
    /// Which download protocols are configured.
    pub protocols: Protocols,
    /// The address services to compare egress against; empty switches leak
    /// detection off.
    pub echo: Vec<String>,
    /// What the download client says it is listening on, where it could be asked.
    /// Read by the caller: this check speaks to containers, not to services.
    pub listening: Option<u16>,
    /// What was asked for around port forwarding.
    pub port_forward: PortForward,
    /// Whether the killswitch may be proven by breaking the tunnel.
    pub disruptive: bool,
    /// The download client to move, where one could be authenticated to.
    ///
    /// Read by the caller for the same reason `listening` is: this check speaks to
    /// containers, and a client's credentials are a service's own business.
    pub client: Option<Qbittorrent>,
}

/// Whether the check applies, and against what.
enum Target {
    /// The pair to compare.
    Pair(Pair),
    /// Torrents are configured and nothing contains them.
    Unprotected,
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
        asked: Asked,
    ) -> Self {
        let Asked {
            protocols,
            echo,
            listening,
            port_forward,
            disruptive,
            client,
        } = asked;
        let target = if protocols.torrent {
            // A pair that will not resolve is two different situations, and only
            // one of them is a skip. A stack with a torrent client and nothing
            // containing it is the arrangement this whole category exists to
            // catch — reported as "does not apply" it reads as though the check
            // found nothing to look at, while what it found is traffic leaving
            // under the operator's own address. A stack with no torrent client at
            // all has nothing to protect, whatever the setting says.
            resolve_pair(manifest).map_or_else(
                || {
                    if torrent_client(manifest) {
                        Target::Unprotected
                    } else {
                        Target::Skip("this stack declares no torrent client to contain".to_owned())
                    }
                },
                Target::Pair,
            )
        } else {
            Target::Skip("torrent downloads are not configured".to_owned())
        };
        // Only a resolved pair has a gateway to read a grant from, and only a stack
        // asking for a forwarded port has one to move the client onto.
        let mender = match (&target, port_forward.enabled) {
            (Target::Pair(pair), true) => Some(mender::PortMender::new(
                engine.clone(),
                project.clone(),
                pair.gateway.clone(),
                client,
            )),
            _ => None,
        };
        Self {
            engine,
            project,
            echo,
            listening,
            target,
            port_forward,
            disruptive,
            mender,
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
}

#[async_trait]
impl Check for VpnCheck {
    fn category(&self) -> Category {
        Category::Vpn
    }

    fn mender(&self) -> Option<&dyn crate::doctor::Mend> {
        self.mender
            .as_ref()
            .map(|mender| mender as &dyn crate::doctor::Mend)
    }

    async fn run(&self) -> Vec<Finding> {
        let pair = match &self.target {
            Target::Skip(reason) => return vec![skipped(reason.clone())],
            Target::Unprotected => return vec![unprotected()],
            Target::Pair(pair) => pair,
        };

        // The engine being unreachable is a reason the checks could not run,
        // never a report that the stack is safe. Leak detection being switched off
        // is an opt-out that still holds here: the operator asked not to be told
        // about egress, so it stays skipped rather than becoming an unverified
        // engine finding — while port forwarding, which they did ask for, reports.
        let Ok(containers) = self.engine.list(&self.project).await else {
            return if self.echo.is_empty() {
                vec![
                    skipped("leak detection is switched off".to_owned()),
                    port_forward_offline(&self.port_forward),
                ]
            } else {
                unreachable_engine(pair, &self.port_forward, self.disruptive)
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
        // A client on the wrong port is a separate fact from a port having been
        // granted, and fails separately — so it is its own finding rather than a
        // clause inside that one.
        let mismatch = match (
            read_grant(self.engine.as_ref(), gateway_container).await,
            self.listening,
        ) {
            (Grant::Port(granted), Some(listening)) if granted != listening => {
                Some(port_mismatch(granted, listening))
            }
            _ => None,
        };

        if self.echo.is_empty() {
            return vec![
                skipped("leak detection is switched off".to_owned()),
                port_forward,
            ];
        }
        // The first source is what the single-answer reads still use — the country
        // and the killswitch probe, neither of which is a comparison and so neither
        // of which gains anything from a second opinion.
        let echo = self.echo.first().map_or("", String::as_str);

        let client_container = find(&containers, &pair.client);

        // Every configured source is asked, so a single one that is wrong cannot
        // make the check say `pass` while traffic leaves in the clear.
        let (gateway, gateway_seen) =
            addresses(self.engine.as_ref(), gateway_container, &self.echo).await;
        let (client, client_seen) =
            addresses(self.engine.as_ref(), client_container, &self.echo).await;

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
        // A disagreement is reported rather than resolved: there is no basis to
        // prefer one stranger's account over another's, and a check that quietly
        // chose would be least trustworthy exactly when it mattered most.
        findings.extend(
            [&gateway_seen, &client_seen]
                .into_iter()
                .filter_map(Seen::said)
                .map(disagreeing),
        );
        findings.push(port_forward);
        findings.extend(mismatch);
        findings
    }
}
