use super::{unprotected, NO_TUNNEL, PORT_MISMATCH};
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
