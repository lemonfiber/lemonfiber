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
use crate::error::Remedy;

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
            &format!("{} tunnel", pair.gateway),
            tunnel_verdict(gateway, &pair.gateway, note),
        ),
        finding(
            "vpn.egress-match",
            &format!("{} egress", pair.client),
            egress_verdict(gateway, client, pair),
        ),
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
    let mut findings = vec![finding(
        "vpn.killswitch",
        "killswitch",
        killswitch::verdict(held),
    )];
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
            killswitch::verdict(&not_attempted(
                disruptive,
                "the container engine could not be reached, so the tunnel could not be dropped",
            )),
        ),
        port_forward_offline(port_forward),
    ]
}
