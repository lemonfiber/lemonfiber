//! The pure leak-detection logic: what a container answered, and the verdict
//! that comparison yields.

use super::Pair;
use crate::doctor::Verdict;
use crate::error::{Code, Problem, Remedy, Severity};

/// Raised when the download client's egress does not match the tunnel.
pub const LEAKING: Code = Code::new("VPN-1");

/// Raised when the VPN container that should carry traffic is not running.
pub const VPN_CONTAINER_DOWN: Code = Code::new("VPN-2");

/// Raised when the client cannot reach the internet through the tunnel.
pub const CLIENT_ISOLATED: Code = Code::new("VPN-3");

/// What a container answered when asked for its public address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Reach {
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

/// The tunnel finding, from the gateway's own answer.
///
/// A container that is not running is a definite fault; a running one that could
/// not reach the endpoint is not, because that is indistinguishable from the
/// endpoint being unreachable — and loss of the oracle is not evidence of a
/// down tunnel any more than of a working one.
pub(super) fn tunnel_verdict(gateway: &Reach, gateway_id: &str, note: Option<String>) -> Verdict {
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
pub(super) fn egress_verdict(gateway: &Reach, client: &Reach, pair: &Pair) -> Verdict {
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
pub(super) fn killswitch_verdict(disruptive: bool) -> Verdict {
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

/// Whether a response is an address rather than an error page.
///
/// Parsed as an address rather than pattern-matched, so an error body that
/// happens to hold dots and hex digits cannot be mistaken for one and shown as
/// the tunnel's egress.
pub(super) fn looks_like_ip(text: &str) -> bool {
    text.parse::<std::net::IpAddr>().is_ok()
}

/// Whether a response is a two- or three-letter country code.
pub(super) fn is_country_code(text: &str) -> bool {
    let letters = text.chars().count();
    (2..=3).contains(&letters) && text.chars().all(|c| c.is_ascii_alphabetic())
}

/// An address, with its country where one was learned.
pub(super) fn labelled(ip: &str, country: Option<String>) -> String {
    match country {
        Some(country) => format!("{ip} ({country})"),
        None => ip.to_owned(),
    }
}
