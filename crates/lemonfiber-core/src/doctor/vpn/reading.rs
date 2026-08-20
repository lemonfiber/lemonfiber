//! What the VPN panel shows.
//!
//! The dashboard asks a different question of the tunnel than the check does: not
//! "is this safe" but "what is it doing". Both rest on the same container reads, so
//! the panel's pass lives beside the check rather than growing its own probes — a
//! panel and a diagnostic disagreeing about the tunnel is the one thing sharing
//! those probes exists to prevent.

use lemonfiber_manifest::Manifest;

use crate::config::Protocols;
use crate::ports::docker::Engine;

use super::leak::Reach;
use super::pair::resolve_pair;
use super::port_forward::Grant;
use super::probe::{addresses, exit_country, find, read_grant};

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
    echoes: Vec<String>,
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

    if echoes.is_empty() {
        return VpnReading::Unavailable(
            "leak detection is switched off, so the tunnel's egress cannot be read".to_owned(),
        );
    }
    // The panel compares the same way the check does, so it reads the same number
    // — a panel and a diagnostic disagreeing about the tunnel is the one thing
    // sharing these probes exists to prevent.
    let (gateway, gateway_seen) = addresses(engine, gateway_container, &echoes).await;
    if let Some(disagreement) = gateway_seen.said() {
        return VpnReading::Unavailable(disagreement);
    }
    let echo = echoes.first().map_or("", String::as_str);
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
            let (client, _) = addresses(engine, find(&containers, &pair.client), &echoes).await;
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
