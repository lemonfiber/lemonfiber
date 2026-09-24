use super::{suppressing, Accepted};
use crate::doctor::{Category, Finding, Verdict};
use crate::error::{Code, Problem, Remedy, Severity, State};

const CODE: Code = Code::new("test.choice");

/// Where each finding stands, as the surface reads it. Total rather than a
/// match with a fallback, so the non-warning arm is exercised by the test
/// about failures rather than left as a branch nothing reaches.
fn states(findings: &[Finding]) -> Vec<Option<State>> {
    findings
        .iter()
        .map(|finding| match &finding.verdict {
            Verdict::Warn(problem) | Verdict::Fail(problem) => Some(problem.state),
            Verdict::Pass { .. } | Verdict::Unverified { .. } | Verdict::Skipped { .. } => None,
        })
        .collect()
}

/// A finding about a choice with a cost.
fn warned(check: &str) -> Finding {
    Finding::in_category(
        Category::Vpn,
        check,
        "Running without a tunnel",
        Verdict::Warn(Problem::new(
            CODE,
            Severity::Warning,
            "No VPN is configured",
            "Torrent traffic leaves this machine under its own address.",
            Remedy::new("Configure a VPN, or accept this deliberately"),
        )),
    )
}

/// A record in which `check` has been answered.
fn answered(check: &str) -> Accepted {
    let mut accepted = Accepted::new();
    accepted.accept(check);
    accepted
}

#[test]
fn a_choice_already_answered_stops_leading() {
    // The same warning every run is not a second warning. An operator who has
    // weighed it learns the tool repeats itself, and stops reading all of it.
    let suppressed = suppressing(vec![warned("vpn.tunnel")], &answered("vpn.tunnel"));
    assert_eq!(states(&suppressed), vec![Some(State::Suppressed)]);
}

#[test]
fn an_acknowledged_choice_is_never_shown_as_fine() {
    // "You chose this" and "this is not happening" are different claims, and
    // only one of them is true. It stays a warning; only its state moves.
    let suppressed = suppressing(vec![warned("vpn.tunnel")], &answered("vpn.tunnel"));
    assert!(suppressed
        .iter()
        .all(|finding| matches!(finding.verdict, Verdict::Warn(_))));
    // A pass would be telling the operator something they can check and find
    // untrue; only the state moves.
    assert_eq!(states(&suppressed), vec![Some(State::Suppressed)]);
}

#[test]
fn a_choice_nobody_answered_is_left_exactly_as_it_was() {
    let untouched = suppressing(vec![warned("vpn.tunnel")], &Accepted::new());
    assert_eq!(states(&untouched), vec![Some(State::Actionable)]);
}

#[test]
fn answering_one_choice_says_nothing_about_another() {
    let other = suppressing(vec![warned("vpn.port-forward")], &answered("vpn.tunnel"));
    assert_eq!(states(&other), vec![Some(State::Actionable)]);
}

#[test]
fn a_failure_is_never_acknowledged_away() {
    // The single most damaging thing this could do. A fault is not a choice,
    // and no answer to it makes it stop being true.
    let failing = Finding::in_category(
        Category::Vpn,
        "vpn.egress",
        "Traffic is behind the tunnel",
        Verdict::Fail(Problem::new(
            CODE,
            Severity::Critical,
            "Traffic is leaving outside the tunnel",
            "Every torrent this machine runs is visible under its own address.",
            Remedy::new("Stop the download client"),
        )),
    );
    let after = suppressing(vec![failing], &answered("vpn.egress"));
    assert!(after
        .iter()
        .all(|finding| matches!(finding.verdict, Verdict::Fail(_))));
    // Still exactly where it was: an answer to a fault changes nothing about it.
    assert_eq!(states(&after), vec![Some(State::Actionable)]);
}

#[test]
fn a_check_that_did_not_apply_is_left_alone_however_it_was_answered() {
    // Nothing was asked of the operator, so there is nothing they answered —
    // and a skipped check turned into a suppressed warning would claim a
    // decision nobody made.
    let skipped = Finding::in_category(
        Category::Vpn,
        "vpn.tunnel",
        "Running without a tunnel",
        Verdict::Skipped {
            reason: "this stack declares no torrent client".to_owned(),
        },
    );
    let after = suppressing(vec![skipped], &answered("vpn.tunnel"));
    assert_eq!(states(&after), vec![None]);
    assert!(after
        .iter()
        .all(|finding| matches!(finding.verdict, Verdict::Skipped { .. })));
}
