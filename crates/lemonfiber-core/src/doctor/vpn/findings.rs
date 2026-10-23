//! Turning what the containers answered into what the operator is told.
//!
//! Kept apart from the asking because the two go wrong in different ways and are
//! read for different reasons: a probe is about reaching a container, and a
//! finding is about the sentence an operator acts on.

use super::killswitch::{self, not_attempted, Held};
use super::leak::Reach;
use super::leak::{egress_verdict, tunnel_verdict};
use super::port_forward::port_forward_offline;
use super::Pair;
use super::{Category, Finding, Verdict};
use crate::config::PortForward;
use crate::error::codes::vpn::{NO_TUNNEL, PORT_MISMATCH};
use crate::error::{Problem, Remedy, State};

pub(super) fn assemble(
    pair: &Pair,
    gateway: &Reach,
    client: &Reach,
    note: Option<String>,
    killswitch: Vec<Finding>,
) -> Vec<Finding> {
    let mut findings = vec![
        finding(
            "vpn.tunnel",
            &format!("{} tunnel", pair.gateway_name),
            tunnel_verdict(gateway, pair, note),
        )
        .about(&pair.gateway),
        finding(
            "vpn.egress-match",
            &format!("{} egress", pair.client_name),
            egress_verdict(gateway, client, pair),
        )
        .about(&pair.client),
    ];
    findings.extend(killswitch);
    findings
}

/// The killswitch finding, and — only where the test broke something it could not
/// put back — a second one saying so.
///
/// Two findings rather than one, because a stack that both leaked and was left
/// without a tunnel has two things wrong with it and folding them into a single
/// verdict would report whichever was written last.
pub(super) fn killswitch_findings(held: &Held) -> Vec<Finding> {
    let mut findings = vec![killswitch::judged(held)];
    if matches!(held, Held::NotRestored) {
        findings.push(finding(
            "vpn.tunnel-restored",
            "tunnel restored after the test",
            killswitch::verdict(&Held::NotRestored),
        ));
    }
    findings
}

pub(super) fn finding(check: &str, title: &str, verdict: Verdict) -> Finding {
    Finding::in_category(Category::Vpn, check, title, verdict)
}

/// A single finding for a check that does not apply.
pub(super) fn skipped(reason: String) -> Finding {
    finding("vpn", "VPN verification", Verdict::Skipped { reason })
}

/// The finding for a stack that torrents with nothing containing it.
///
/// A warning rather than a failure, and deliberately so: nothing is broken, and
/// running this way can be a decision — which is why it can be answered once
/// (`lemonfiber doctor --accept vpn.unprotected`) and stops leading afterwards. A
/// failure could not be, and one that could would be the most damaging thing in
/// this file.
///
/// Its own check rather than the tunnel's, because "there is no tunnel" and "the
/// tunnel is unhealthy" are different claims, and an answer to the first must
/// never quieten the second.
pub(super) fn unprotected() -> Finding {
    finding(
        "vpn.unprotected",
        "Torrent traffic is contained",
        Verdict::Warn(
            Problem::new(
                NO_TUNNEL,
                "Torrent traffic is not contained by a VPN",
                "This stack declares torrents and no VPN-contained client to run them \
                 through, so every torrent it runs is visible under this connection's own \
                 address — to the network it is on, and to every other peer in the swarm.",
                Remedy::new("Put the torrent client behind a VPN container in the stack")
                    .with_detail(
                        "or, where this is deliberate: lemonfiber doctor --accept vpn.unprotected",
                    ),
            )
            .in_state(crate::error::State::Guided),
        ),
    )
}

/// The finding for address services that contradict each other.
///
/// Unverified rather than a failure: nothing here says traffic is leaking. It
/// says the one number every other verdict is compared against cannot be
/// established, which is a different and more honest claim — and one the operator
/// can act on, since the fix is to their sources rather than to their tunnel.
pub(super) fn disagreeing(reason: String) -> Finding {
    finding(
        "vpn.egress-sources",
        "Address services agree",
        Verdict::Unverified {
            reason,
            remedy: Remedy::new(
                "Check the address services are reachable and not behind a caching proxy",
            )
            .with_detail("LEMONFIBER_IP_ECHO accepts several, comma-separated"),
        },
    )
}

/// The finding for a download client listening somewhere other than the port the
/// provider granted, which the diagnosis passes to it once it has run.
///
/// A warning rather than a failure — nothing is leaking and downloads still
/// arrive. What stops is peers reaching the client, so seeding stops, which is
/// the part noticed last and the reason this is worth saying at all.
pub(super) fn port_mismatch(granted: u16, listening: u16) -> Finding {
    finding(
        "vpn.port-forward-client",
        "The download client listens on the forwarded port",
        Verdict::Warn(
            Problem::new(
                PORT_MISMATCH,
                format!(
                    "The provider forwards port {granted} and the download client is listening \
                     on {listening}"
                ),
                "Downloads still arrive, so nothing looks wrong from here — but no peer can \
                 reach the client, so it cannot seed and connects to fewer sources.",
                Remedy::new(
                    "Run the diagnosis again: it passes the forwarded port to the client \
                     once it has run, and says what it set",
                )
                .with_detail("lemonfiber doctor"),
            )
            .or_try(Remedy::new(format!(
                "Or set the client's listening port to {granted} yourself"
            )))
            .in_state(State::Remediable),
        ),
    )
}

/// The findings when the engine could not be reached: the runtime checks could
/// not run, so they are unverified rather than reported either way.
pub(super) fn unreachable_engine(
    pair: &Pair,
    port_forward: &PortForward,
    disruptive: bool,
) -> Vec<Finding> {
    let reason = "the container engine could not be reached, so the containers \
                  could not be asked"
        .to_owned();
    let remedy = Remedy::new("Start the container engine, then run this again");
    vec![
        finding(
            "vpn.tunnel",
            &format!("{} tunnel", pair.gateway_name),
            Verdict::Unverified {
                reason: reason.clone(),
                remedy: remedy.clone(),
            },
        )
        .about(&pair.gateway),
        finding(
            "vpn.egress-match",
            &format!("{} egress", pair.client_name),
            Verdict::Unverified { reason, remedy },
        )
        .about(&pair.client),
        killswitch::judged(&not_attempted(
            disruptive,
            "the container engine could not be reached, so the tunnel could not be dropped",
        )),
        port_forward_offline(port_forward),
    ]
}

#[cfg(test)]
mod tests;
