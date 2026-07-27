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
use crate::config::Protocols;
use crate::error::{Code, Problem, Remedy, Severity};
use crate::ports::docker::{Container, Engine, Lifecycle};

/// Raised when the download client's egress does not match the tunnel.
pub const LEAKING: Code = Code::new("VPN-1");

/// Raised when the VPN container that should carry traffic is not running.
pub const VPN_CONTAINER_DOWN: Code = Code::new("VPN-2");

/// Raised when the client cannot reach the internet through the tunnel.
pub const CLIENT_ISOLATED: Code = Code::new("VPN-3");

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

/// The VPN leak check: compare the client's egress against the tunnel's.
pub struct VpnCheck {
    engine: Arc<dyn Engine>,
    project: String,
    echo: Option<String>,
    target: Target,
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
        let Some(echo) = self.echo.as_deref() else {
            return vec![skipped("leak detection is switched off".to_owned())];
        };

        // The engine being unreachable is a reason the check could not run,
        // never a report that the stack is safe.
        let Ok(containers) = self.engine.list(&self.project).await else {
            return unreachable_engine(pair, self.disruptive);
        };
        if containers.is_empty() {
            return vec![skipped("the stack is not running".to_owned())];
        }

        let gateway_container = find(&containers, &pair.gateway);
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

        assemble(pair, &gateway, &client, note, self.disruptive)
    }
}

/// The command that asks an endpoint for the caller's address, run inside a
/// container.
fn wget(url: String) -> Vec<String> {
    vec!["wget".to_owned(), "-qO-".to_owned(), url]
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
fn unreachable_engine(pair: &Pair, disruptive: bool) -> Vec<Finding> {
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
    ]
}
