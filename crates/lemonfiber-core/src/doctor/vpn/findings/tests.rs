use super::{assemble, unprotected, unreachable_engine, NO_TUNNEL, PORT_MISMATCH};
use crate::doctor::vpn::leak::Reach;
use crate::doctor::vpn::Pair;
use crate::doctor::Verdict;

#[test]
fn every_vpn_problem_has_its_own_code() {
    // A code is what an operator searches for. Two different problems sharing
    // one sends them to the wrong explanation — which is what happened here:
    // the port mismatch and the killswitch leak were both VPN-5.
    let codes = [
        crate::error::codes::vpn::NO_FORWARDED_PORT,
        PORT_MISMATCH,
        NO_TUNNEL,
        crate::error::codes::vpn::KILLSWITCH_LEAKS,
        crate::error::codes::vpn::TUNNEL_NOT_RESTORED,
        crate::error::codes::vpn::LEAKING,
        crate::error::codes::vpn::VPN_CONTAINER_DOWN,
        crate::error::codes::vpn::CLIENT_ISOLATED,
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

/// The tunnel and the client, as the manifest names them.
fn pair() -> Pair {
    Pair {
        gateway: "gluetun".to_owned(),
        client: "qbittorrent".to_owned(),
        gateway_name: "Gluetun".to_owned(),
        client_name: "qBittorrent".to_owned(),
    }
}

/// A tunnel that is down is written about as Gluetun and reported against gluetun: the
/// words name the service as the operator knows it, the finding says which service it
/// is about, and the command in the remedy keeps the id the command takes.
#[test]
fn a_down_tunnel_names_its_service_as_the_operator_knows_it() {
    let findings = assemble(
        &pair(),
        &Reach::Down,
        &Reach::Address("203.0.113.9".to_owned()),
        None,
        Vec::new(),
    );
    let titles: Vec<(&str, Option<&str>)> = findings
        .iter()
        .map(|finding| (finding.title.as_str(), finding.service.as_deref()))
        .collect();
    assert_eq!(
        titles,
        vec![
            ("Gluetun tunnel", Some("gluetun")),
            ("qBittorrent egress", Some("qbittorrent")),
        ]
    );

    let tunnel = findings.first().and_then(problem_of);
    assert_eq!(
        tunnel.map(|problem| problem.summary.as_str()),
        Some("The VPN container Gluetun is not running")
    );
    assert_eq!(
        tunnel
            .and_then(|problem| problem.remedies.first())
            .and_then(|remedy| remedy.detail.as_deref()),
        Some("lemonfiber logs gluetun")
    );
    let egress = findings.get(1).and_then(problem_of);
    assert_eq!(
        egress.map(|problem| problem.summary.as_str()),
        Some("qBittorrent has connectivity the VPN does not")
    );
}

/// Where the engine could not be asked, the two findings it could not settle still
/// carry the names and the services they would have been about.
#[test]
fn an_unreachable_engine_still_names_both_services() {
    let findings = unreachable_engine(&pair(), &crate::config::PortForward::default(), false);
    let titles: Vec<(&str, Option<&str>)> = findings
        .iter()
        .take(2)
        .map(|finding| (finding.title.as_str(), finding.service.as_deref()))
        .collect();
    assert_eq!(
        titles,
        vec![
            ("Gluetun tunnel", Some("gluetun")),
            ("qBittorrent egress", Some("qbittorrent")),
        ]
    );
}
