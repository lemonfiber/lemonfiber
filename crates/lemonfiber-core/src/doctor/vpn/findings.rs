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
use crate::error::{Code, Problem, Remedy, Severity};

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
                Severity::Warning,
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
/// provider granted.
///
/// Offered rather than applied, because a diagnosis is only looking: an operator
/// asking what is wrong has not asked for anything to be changed. Starting the
/// stack applies the same fix, since by then they have asked for an action.
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
                Severity::Warning,
                format!(
                    "The provider forwards port {granted} and the download client is listening \
                     on {listening}"
                ),
                "Downloads still arrive, so nothing looks wrong from here — but no peer can \
                 reach the client, so it cannot seed and connects to fewer sources.",
                Remedy::new("Start the stack to move the client onto the forwarded port")
                    .with_detail("lemonfiber up"),
            )
            .or_try(Remedy::new(format!(
                "Or set the client's listening port to {granted} yourself"
            ))),
        ),
    )
}

/// Raised when the client is listening somewhere other than the forwarded port.
pub const PORT_MISMATCH: Code = Code::new("VPN-7");

/// Raised when torrents are configured with nothing containing them.
pub const NO_TUNNEL: Code = Code::new("VPN-8");

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

#[cfg(test)]
mod tests {
    use super::{unprotected, NO_TUNNEL, PORT_MISMATCH};
    use crate::doctor::Verdict;

    #[test]
    fn every_vpn_problem_has_its_own_code() {
        // A code is what an operator searches for. Two different problems sharing
        // one sends them to the wrong explanation — which is what happened here:
        // the port mismatch and the killswitch leak were both VPN-5.
        let codes = [
            crate::doctor::vpn::NO_FORWARDED_PORT,
            PORT_MISMATCH,
            NO_TUNNEL,
            super::super::killswitch::KILLSWITCH_LEAKS,
            super::super::killswitch::TUNNEL_NOT_RESTORED,
            super::super::leak::LEAKING,
            super::super::leak::VPN_CONTAINER_DOWN,
            super::super::leak::CLIENT_ISOLATED,
        ];
        let mut distinct: Vec<&str> = codes.iter().map(|code| code.as_str()).collect();
        distinct.sort_unstable();
        let counted = distinct.len();
        distinct.dedup();
        assert_eq!(distinct.len(), counted, "{distinct:?}");
    }

    /// The problem a finding carries, where it carries one. Total rather than a
    /// match with a fallback, so the other arm is exercised by the skip below
    /// rather than left as a branch nothing reaches.
    fn problem_of(finding: &crate::doctor::Finding) -> Option<&crate::error::Problem> {
        match &finding.verdict {
            Verdict::Warn(problem) | Verdict::Fail(problem) => Some(problem),
            Verdict::Pass { .. } | Verdict::Unverified { .. } | Verdict::Skipped { .. } => None,
        }
    }

    #[test]
    fn the_uncontained_warning_says_what_it_costs_and_how_to_answer_it() {
        // Both halves matter: a warning that cannot be answered is one an operator
        // has to keep reading for ever, and a warning that says only "no VPN"
        // leaves them guessing whether it matters.
        let finding = unprotected();
        assert!(
            matches!(finding.verdict, Verdict::Warn(_)),
            "never a failure"
        );
        let meaning = problem_of(&finding).map(|problem| problem.meaning.clone());
        assert!(
            meaning.is_some_and(|meaning| meaning.contains("visible")),
            "it says what running this way costs"
        );
        let detail = problem_of(&finding)
            .and_then(|problem| problem.remedies.first())
            .and_then(|remedy| remedy.detail.clone())
            .unwrap_or_default();
        assert!(detail.contains("--accept vpn.unprotected"), "{detail}");

        // And a skip carries nothing to say, which is what makes the reading above
        // a total one rather than a match with somewhere to hide.
        assert!(problem_of(&super::skipped("nothing to contain".to_owned())).is_none());
    }
}
