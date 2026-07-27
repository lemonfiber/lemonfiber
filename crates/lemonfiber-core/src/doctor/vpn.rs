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

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_manifest::{Manifest, Protocol};

use super::{Category, Check, Finding, Verdict};
use crate::config::{PortForward, Protocols};
use crate::error::{Code, Problem, Remedy, Severity, State};
use crate::ports::docker::{Container, Engine, Lifecycle};

/// Raised when the download client's egress does not match the tunnel.
pub const LEAKING: Code = Code::new("VPN-1");

/// Raised when the VPN container that should carry traffic is not running.
pub const VPN_CONTAINER_DOWN: Code = Code::new("VPN-2");

/// Raised when the client cannot reach the internet through the tunnel.
pub const CLIENT_ISOLATED: Code = Code::new("VPN-3");

/// Raised when port forwarding was asked for but the provider granted no port.
pub const NO_FORWARDED_PORT: Code = Code::new("VPN-4");

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

/// What a container answered when asked for its public address.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Reach {
    /// A usable public address.
    Address(String),
    /// The command ran but returned no usable address — no connectivity, or an
    /// unreadable answer.
    Blocked,
    /// The container is not running, so it could not be asked.
    Down,
    /// The command could not be run at all.
    Unknown,
}

/// What the gateway's forwarded-port status file amounted to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grant {
    /// A port was granted.
    Port(u16),
    /// The file was read but names no usable port, so none was granted.
    Absent,
    /// The file could not be read: the container is down or the engine is
    /// unreachable, so whether a port exists is unknown rather than absent.
    Unreadable,
}

/// How much lemonfiber knows about a provider's port forwarding, which decides
/// what a missing port can honestly be called.
enum Knowledge {
    /// `ProtonVPN`, whose trap — port forwarding and a P2P server both chosen when
    /// the `WireGuard` configuration is generated — is specific and nameable.
    Proton,
    /// A provider known to offer port forwarding, but without a lemonfiber-specific
    /// trap to name beyond the generic one.
    Forwarding,
    /// A provider lemonfiber has no port-forwarding knowledge of, so a missing
    /// port cannot be explained without guessing.
    Unknown,
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
        let Some(container) = container else {
            return Reach::Down;
        };
        if container.lifecycle != Lifecycle::Running {
            return Reach::Down;
        }
        match self
            .engine
            .exec(&container.id, &wget(echo.to_owned()))
            .await
        {
            Err(_) => Reach::Unknown,
            Ok(output) => {
                let body = output.stdout.trim();
                if output.status == Some(0) && looks_like_ip(body) {
                    Reach::Address(body.to_owned())
                } else {
                    Reach::Blocked
                }
            }
        }
    }

    /// The tunnel's exit country, best effort — reported where the endpoint can
    /// supply it, omitted rather than guessed where it cannot.
    async fn country(&self, container: &Container, echo: &str) -> Option<String> {
        let url = format!("{}/country-iso", echo.trim_end_matches('/'));
        match self.engine.exec(&container.id, &wget(url)).await {
            Ok(output) if output.status == Some(0) && is_country_code(output.stdout.trim()) => {
                Some(output.stdout.trim().to_ascii_uppercase())
            }
            _ => None,
        }
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

    /// Read the granted port from the gateway's own status file.
    ///
    /// A container that is not running, or an engine that cannot be reached, makes
    /// the answer unknown rather than absent. A file that reads as no port — empty,
    /// missing, or the zero the release path writes — is a port that was not
    /// granted, which for an enabled provider is the failure this check exists for.
    async fn granted_port(&self, gateway: Option<&Container>) -> Grant {
        let Some(container) = gateway else {
            return Grant::Unreadable;
        };
        if container.lifecycle != Lifecycle::Running {
            return Grant::Unreadable;
        }
        // Awaited into a plain value first: a block whose last statement is an
        // await leaves its own closing brace unmarked by coverage.
        let result = self.engine.exec(&container.id, &read_port()).await;
        match result {
            Err(_) => Grant::Unreadable,
            Ok(output) => parse_grant(&output.stdout),
        }
    }
}

/// The findings for an active check, given both containers' answers.
fn assemble(
    pair: &Pair,
    gateway: &Reach,
    client: &Reach,
    note: Option<String>,
    disruptive: bool,
) -> Vec<Finding> {
    vec![
        finding(
            "vpn.tunnel",
            &format!("{} tunnel", pair.gateway),
            tunnel_verdict(gateway, &pair.gateway, note),
        ),
        finding(
            "vpn.egress-match",
            &format!("{} egress", pair.client),
            egress_verdict(gateway, client, pair),
        ),
        finding(
            "vpn.killswitch",
            "killswitch",
            killswitch_verdict(disruptive),
        ),
    ]
}

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

        let mut findings = assemble(pair, &gateway, &client, note, self.disruptive);
        findings.push(port_forward);
        findings
    }
}

/// The command that asks an endpoint for the caller's address, run inside a
/// container.
fn wget(url: String) -> Vec<String> {
    vec!["wget".to_owned(), "-qO-".to_owned(), url]
}

/// The command that reads the gateway's forwarded-port status file from inside
/// the container.
fn read_port() -> Vec<String> {
    vec!["cat".to_owned(), FORWARDED_PORT_FILE.to_owned()]
}

/// Read the status file's contents as a granted port.
///
/// A file that is missing makes `cat` exit non-zero with nothing on stdout, and
/// the release path writes a literal `0`; both are read as no port granted rather
/// than as a failure to read, because the file was reachable — it simply names no
/// port. Only an engine or container fault, handled before this, is `Unreadable`.
fn parse_grant(stdout: &str) -> Grant {
    match stdout.trim().parse::<u16>() {
        Ok(port) if port != 0 => Grant::Port(port),
        _ => Grant::Absent,
    }
}

/// The verdict for an enabled provider that granted no port.
///
/// A provider lemonfiber knows forwards ports gets a failure — degraded, not
/// broken: nothing is leaking, but peers cannot reach the client. `ProtonVPN`'s
/// specific trap is named first where it is the provider. A provider lemonfiber
/// has no knowledge of is left `unverified` rather than blamed, because the cause
/// cannot be named without guessing.
fn no_port(provider: Option<&str>) -> Verdict {
    match knowledge(provider) {
        Knowledge::Proton => Verdict::Warn(proton_trap()),
        Knowledge::Forwarding => Verdict::Warn(generic_trap()),
        Knowledge::Unknown => Verdict::Unverified {
            reason: "port forwarding is enabled but no port was granted, and this is not a \
                     provider lemonfiber has specific guidance for, so the cause cannot be named \
                     without guessing"
                .to_owned(),
            remedy: Remedy::new(
                "Confirm your provider supports port forwarding and that it was enabled when the \
                 VPN credentials were generated",
            ),
        },
    }
}

/// How much lemonfiber knows about a provider, by the name Gluetun uses for it.
fn knowledge(provider: Option<&str>) -> Knowledge {
    match provider {
        Some("protonvpn" | "proton") => Knowledge::Proton,
        Some(
            "private internet access"
            | "pia"
            | "privateinternetaccess"
            | "privatevpn"
            | "perfect privacy"
            | "perfectprivacy",
        ) => Knowledge::Forwarding,
        _ => Knowledge::Unknown,
    }
}

/// `ProtonVPN`'s trap, named first: the tunnel connects but the port never arrives
/// because forwarding was not chosen at configuration time.
fn proton_trap() -> Problem {
    Problem::new(
        NO_FORWARDED_PORT,
        Severity::Warning,
        "The VPN granted no forwarded port",
        "The tunnel is up, but no port was forwarded, so peers cannot open connections to your \
         client and both download connectivity and seeding are reduced. With ProtonVPN the usual \
         cause is that port forwarding (NAT-PMP) was not enabled, or a non-P2P server was chosen, \
         when the WireGuard configuration was generated — the tunnel still connects, only the port \
         never arrives. It cannot be fixed at runtime.",
        Remedy::new(
            "Regenerate the ProtonVPN WireGuard credentials with NAT-PMP enabled and a P2P server, \
             then replace WIREGUARD_PRIVATE_KEY",
        )
        .with_detail("account.protonvpn.com → Downloads → WireGuard: enable NAT-PMP, pick a P2P server"),
    )
    .in_state(State::Guided)
}

/// The trap for a provider that forwards ports but has no lemonfiber-specific
/// note: state the consequence and where the setting usually lives, without
/// inventing a cause.
fn generic_trap() -> Problem {
    Problem::new(
        NO_FORWARDED_PORT,
        Severity::Warning,
        "The VPN granted no forwarded port",
        "The tunnel is up, but no port was forwarded, so peers cannot open connections to your \
         client and both download connectivity and seeding are reduced. On providers that support \
         port forwarding it usually has to be enabled at the point the credentials are generated, \
         not afterwards.",
        Remedy::new(
            "Confirm port forwarding is enabled for this provider, and regenerate the VPN \
             credentials with it enabled if it was not",
        ),
    )
    .in_state(State::Guided)
}

/// Whether a response is an address rather than an error page.
///
/// Parsed as an address rather than pattern-matched, so an error body that
/// happens to hold dots and hex digits cannot be mistaken for one and shown as
/// the tunnel's egress.
fn looks_like_ip(text: &str) -> bool {
    text.parse::<std::net::IpAddr>().is_ok()
}

/// Whether a response is a two- or three-letter country code.
fn is_country_code(text: &str) -> bool {
    let letters = text.chars().count();
    (2..=3).contains(&letters) && text.chars().all(|c| c.is_ascii_alphabetic())
}

/// An address, with its country where one was learned.
fn labelled(ip: &str, country: Option<String>) -> String {
    match country {
        Some(country) => format!("{ip} ({country})"),
        None => ip.to_owned(),
    }
}

/// The container implementing a service, where it is present.
fn find<'a>(containers: &'a [Container], service: &str) -> Option<&'a Container> {
    containers
        .iter()
        .find(|container| container.service == service)
}

/// A finding in the VPN category.
fn finding(check: &str, title: &str, verdict: Verdict) -> Finding {
    Finding {
        check: check.to_owned(),
        category: Category::Vpn,
        title: title.to_owned(),
        verdict,
    }
}

/// A single finding for a check that does not apply.
fn skipped(reason: String) -> Finding {
    finding("vpn", "VPN verification", Verdict::Skipped { reason })
}

/// The tunnel finding, from the gateway's own answer.
///
/// A container that is not running is a definite fault; a running one that could
/// not reach the endpoint is not, because that is indistinguishable from the
/// endpoint being unreachable — and loss of the oracle is not evidence of a
/// down tunnel any more than of a working one.
fn tunnel_verdict(gateway: &Reach, gateway_id: &str, note: Option<String>) -> Verdict {
    match gateway {
        Reach::Address(_) => Verdict::Pass { note },
        Reach::Down => Verdict::Fail(
            Problem::new(
                VPN_CONTAINER_DOWN,
                Severity::Error,
                format!("The VPN container {gateway_id} is not running"),
                "Nothing routes through a tunnel that is not up. Torrents cannot \
                 transfer, though nothing is leaking while it is down.",
                Remedy::new("Start the form that includes it, then check its logs")
                    .with_detail(format!("lemonfiber logs {gateway_id}")),
            )
            .in_state(crate::error::State::Guided),
        ),
        Reach::Blocked | Reach::Unknown => Verdict::Unverified {
            reason: "the VPN container did not return an address, which the tunnel \
                     being down and the check service being unreachable both produce"
                .to_owned(),
            remedy: Remedy::new("Check the tunnel is up, then run this again")
                .with_detail(format!("lemonfiber logs {gateway_id}")),
        },
    }
}

/// The egress finding: the leak check itself.
///
/// A leak is defined by the client reaching the internet outside the tunnel, so
/// the client having an address the tunnel does not share is the critical case —
/// never the tunnel's state on its own.
fn egress_verdict(gateway: &Reach, client: &Reach, pair: &Pair) -> Verdict {
    match client {
        Reach::Address(client_ip) => match gateway {
            Reach::Address(gateway_ip) if gateway_ip == client_ip => Verdict::Pass {
                note: Some(client_ip.clone()),
            },
            Reach::Address(_) => Verdict::Fail(mismatch(pair)),
            // The tunnel down or reachable-but-blocked, while the client reached
            // the internet, is the client going around it — a real leak. The
            // tunnel merely unaskable is not: its address is unknown, not proven
            // absent, so a leak cannot be established and is never claimed.
            Reach::Down | Reach::Blocked => Verdict::Fail(uncontained(pair)),
            Reach::Unknown => Verdict::Unverified {
                reason: format!(
                    "{} returned an address, but {}'s own address could not be read, so egress \
                     could not be compared",
                    pair.client, pair.gateway
                ),
                remedy: Remedy::new("Confirm the tunnel is up, then run this again"),
            },
        },
        Reach::Blocked => match gateway {
            Reach::Address(_) => Verdict::Warn(isolated(pair)),
            _ => Verdict::Unverified {
                reason: "neither the client nor the tunnel returned an address, so \
                         egress could not be confirmed — this may be the killswitch \
                         holding, or the check service being unreachable"
                    .to_owned(),
                remedy: Remedy::new("Confirm the tunnel is up, then run this again"),
            },
        },
        Reach::Down => Verdict::Skipped {
            reason: format!(
                "the download client {} is not running, so its egress could not be compared",
                pair.client
            ),
        },
        Reach::Unknown => Verdict::Unverified {
            reason: format!("the check could not be run inside {}", pair.client),
            remedy: Remedy::new("Confirm the client is running, then run this again"),
        },
    }
}

/// The critical finding for a client whose address differs from the tunnel's.
fn mismatch(pair: &Pair) -> Problem {
    Problem::new(
        LEAKING,
        Severity::Critical,
        format!("{}'s traffic is not going through the VPN", pair.client),
        "Its public address does not match the tunnel's, so peers in every swarm \
         can see your home address. This is the one failure whose consequences \
         reach outside your machine.",
        Remedy::new("Stop torrent transfers now, then confirm the client shares the VPN's network")
            .with_detail(format!("network_mode: service:{}", pair.gateway)),
    )
    .in_state(crate::error::State::Guided)
}

/// The critical finding for a client reaching the internet the tunnel could not.
fn uncontained(pair: &Pair) -> Problem {
    Problem::new(
        LEAKING,
        Severity::Critical,
        format!("{} has connectivity the VPN does not", pair.client),
        "The client reached the internet while the tunnel did not, so its traffic \
         is not being carried by the VPN. Your home address is exposed to peers.",
        Remedy::new("Stop torrent transfers now, then confirm the client shares the VPN's network")
            .with_detail(format!("network_mode: service:{}", pair.gateway)),
    )
    .in_state(crate::error::State::Guided)
}

/// The finding for a contained client that currently has no connectivity.
fn isolated(pair: &Pair) -> Problem {
    Problem::new(
        CLIENT_ISOLATED,
        Severity::Warning,
        format!("{} has no connectivity", pair.client),
        "The tunnel is up but the client could not reach the internet through it. \
         Nothing is leaking, but torrents will not transfer until it can.",
        Remedy::new("Confirm the client uses the VPN container's network")
            .with_detail(format!("network_mode: service:{}", pair.gateway)),
    )
    .in_state(crate::error::State::Guided)
}

/// The killswitch finding: unverified either way, because proving a killswitch
/// works means breaking the tunnel and confirming traffic stops.
///
/// An untested fail-closed guarantee reported as passing would be exactly the
/// comfortable falsehood this feature exists to eliminate, so it is never a pass.
/// Where the operator opted into the disruptive checks, it says plainly that the
/// tunnel-drop test is not yet built rather than pointing back at the flag they
/// already gave — a remedy that led in a circle would be worse than an honest gap.
fn killswitch_verdict(disruptive: bool) -> Verdict {
    if disruptive {
        return Verdict::Unverified {
            reason: "the disruptive killswitch test — dropping the tunnel to confirm \
                     traffic stops — is not yet built"
                .to_owned(),
            remedy: Remedy::new(
                "Until it lands, confirm the tunnel container's own killswitch is enabled",
            ),
        };
    }
    Verdict::Unverified {
        reason: "the killswitch has not been tested; proving it works means dropping \
                 the tunnel and confirming traffic stops, which interrupts transfers"
            .to_owned(),
        remedy: Remedy::new("Run the disruptive check when transfers can be interrupted")
            .with_detail("lemonfiber doctor --only vpn --disruptive"),
    }
}

/// The findings when the engine could not be reached: the runtime checks could
/// not run, so they are unverified rather than reported either way.
fn unreachable_engine(pair: &Pair, port_forward: &PortForward, disruptive: bool) -> Vec<Finding> {
    let reason = "the container engine could not be reached, so the containers \
                  could not be asked"
        .to_owned();
    let remedy = Remedy::new("Start the container engine, then run this again");
    vec![
        finding(
            "vpn.tunnel",
            &format!("{} tunnel", pair.gateway),
            Verdict::Unverified {
                reason: reason.clone(),
                remedy: remedy.clone(),
            },
        ),
        finding(
            "vpn.egress-match",
            &format!("{} egress", pair.client),
            Verdict::Unverified { reason, remedy },
        ),
        finding(
            "vpn.killswitch",
            "killswitch",
            killswitch_verdict(disruptive),
        ),
        port_forward_offline(port_forward),
    ]
}

/// The port-forward finding when the gateway cannot be read at all, because the
/// engine is unreachable: unverified where a port was expected, and still just
/// not-applicable where forwarding was never enabled.
fn port_forward_offline(port_forward: &PortForward) -> Finding {
    let verdict = if port_forward.enabled {
        Verdict::Unverified {
            reason: "the container engine could not be reached, so the forwarded port \
                     could not be read"
                .to_owned(),
            remedy: Remedy::new("Start the container engine, then run this again"),
        }
    } else {
        Verdict::Skipped {
            reason: NOT_ENABLED.to_owned(),
        }
    };
    finding("vpn.port-forward", "forwarded port", verdict)
}
