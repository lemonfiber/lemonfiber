use super::{against, standing, Standing, Verification};
use crate::doctor::{Category, Finding, Verdict};
use crate::error::{Code, Problem, Remedy, Severity};

/// A finding under a named check, which is the key the whole rule turns on.
fn finding(check: &str, verdict: Verdict) -> Finding {
    Finding {
        check: check.to_owned(),
        category: Category::Network,
        title: format!("what {check} establishes"),
        service: None,
        caused_by: None,
        said: None,
        verdict,
        origin: crate::origin::Origin::Bundled,
    }
}

/// A verdict that is raising something, at either height.
fn raised(failing: bool) -> Verdict {
    let problem = Problem::new(
        Code::new("TEST-1"),
        Severity::Error,
        "It broke",
        "The thing did not happen",
        Remedy::new("Try again"),
    );
    if failing {
        Verdict::Fail(problem)
    } else {
        Verdict::Warn(problem)
    }
}

/// A verdict that says nothing could be established.
fn untold() -> Verdict {
    Verdict::Unverified {
        reason: "nothing answered".to_owned(),
        remedy: Remedy::new("Try again"),
    }
}

/// The checks a run names, in the order the doctor produced them.
fn named(verification: &[super::Changed]) -> Vec<String> {
    verification
        .iter()
        .map(|one| one.now.check.clone())
        .collect()
}

/// The whole of what the rule buys: a check the install made worse is named, and
/// one that was already failing is not — which is the difference between *this
/// install broke it* and *this machine was already like that*.
#[test]
fn a_check_this_install_made_worse_is_named_and_one_already_failing_is_not() {
    let before = vec![
        finding("network.bindings", Verdict::Pass { note: None }),
        finding("providers.usenet", raised(true)),
    ];
    let after = vec![
        finding("network.bindings", raised(true)),
        finding("providers.usenet", raised(true)),
    ];

    let verification = against(&before, &after);
    assert_eq!(named(&verification.broke), vec!["network.bindings"]);
    assert!(verification.unsettled.is_empty());
    assert!(!verification.held(), "a broken check stops the install");
}

/// A warning the install turns into a failure is the install making it worse, and
/// a ladder of two rungs is what catches it. One rung for *raising something* would
/// call this no change at all.
#[test]
fn a_warning_this_install_turned_into_a_failure_is_named() {
    let before = vec![finding("storage.space", raised(false))];
    let after = vec![finding("storage.space", raised(true))];

    let verification = against(&before, &after);
    assert_eq!(named(&verification.broke), vec!["storage.space"]);
}

/// A check that goes quiet is not an install to reverse, and neither is a check
/// that was not raised before and holds now.
#[test]
fn a_check_that_stopped_being_raised_is_no_event_at_all() {
    let before = vec![
        finding("storage.space", raised(true)),
        finding("environment.engine", raised(false)),
    ];
    let after = vec![finding("storage.space", Verdict::Pass { note: None })];

    let verification = against(&before, &after);
    assert!(
        verification.broke.is_empty(),
        "an improvement is not a break"
    );
    assert!(verification.unsettled.is_empty());
    assert!(verification.held());
}

/// A check nothing raised beforehand and that fails now is this install's doing,
/// because absent reads as holding — which is the same rule the repair's proof
/// takes, read backwards.
#[test]
fn a_check_that_was_not_raised_before_and_fails_now_counts_against_the_install() {
    let after = vec![finding("network.bindings", raised(true))];

    let verification = against(&[], &after);
    assert_eq!(named(&verification.broke), vec!["network.bindings"]);
}

/// Assurance that was there and is not says so, and does not stop the install: the
/// check is not failing, it is saying it could not tell, and reversing on that
/// would punish a plugin for a provider that was unreachable for a moment.
#[test]
fn a_check_that_could_no_longer_be_told_is_unsettled_rather_than_broken() {
    let before = vec![finding("providers.usenet", Verdict::Pass { note: None })];
    let after = vec![finding("providers.usenet", untold())];

    let verification = against(&before, &after);
    assert!(verification.broke.is_empty());
    assert_eq!(named(&verification.unsettled), vec!["providers.usenet"]);
    assert!(
        verification.held(),
        "what could not be told does not take an install back"
    );
}

/// And the mirror: failing now, with nothing beforehand to say whether it was.
/// Naming the install as the cause would be a guess with a reversal attached.
#[test]
fn a_check_that_could_not_be_told_before_and_fails_now_is_unsettled_too() {
    let before = vec![finding("guides.reachable", untold())];
    let after = vec![finding("guides.reachable", raised(true))];

    let verification = against(&before, &after);
    assert!(verification.broke.is_empty());
    assert_eq!(named(&verification.unsettled), vec!["guides.reachable"]);
    assert_eq!(
        verification
            .unsettled
            .first()
            .and_then(|one| one.before.clone()),
        Some(untold()),
        "how it read before is carried, so the reader can see there was nothing to compare"
    );
}

/// One that could not be told before and cannot be told now has not changed, and
/// one that could not be told before and holds now is an improvement. Neither is
/// worth a line.
#[test]
fn what_could_never_be_told_and_what_came_right_are_both_silent() {
    let before = vec![
        finding("guides.reachable", untold()),
        finding("vpn.egress-match", untold()),
    ];
    let after = vec![
        finding("guides.reachable", untold()),
        finding("vpn.egress-match", Verdict::Pass { note: None }),
    ];

    let verification = against(&before, &after);
    assert!(verification.broke.is_empty());
    assert!(verification.unsettled.is_empty());
}

/// A check that did not apply to this machine found nothing wrong with it, so it
/// holds. Reading skipped as anything else would have an install reversed by a
/// check that declined to run.
#[test]
fn a_skipped_check_holds_and_a_run_that_skips_more_is_not_a_break() {
    let before = vec![finding("services.releases", Verdict::Pass { note: None })];
    let after = vec![finding(
        "services.releases",
        Verdict::Skipped {
            reason: "not asked for".to_owned(),
        },
    )];

    assert_eq!(
        after.first().and_then(|one| standing(&one.verdict)),
        Some(Standing::Held)
    );
    assert!(against(&before, &after).held());
}

/// Every verdict the doctor can reach has a rung or says it has none, and the one
/// without is the one that must not be comparable to the others.
#[test]
fn every_verdict_says_where_it_stands_or_that_it_cannot() {
    assert_eq!(
        standing(&Verdict::Pass { note: None }),
        Some(Standing::Held)
    );
    assert_eq!(standing(&raised(false)), Some(Standing::Raised));
    assert_eq!(standing(&raised(true)), Some(Standing::Broken));
    assert_eq!(standing(&untold()), None);
    assert!(Standing::Held < Standing::Raised && Standing::Raised < Standing::Broken);
}

/// Nothing changed is what an install needs, and it is the common answer.
#[test]
fn a_run_that_changed_nothing_holds() {
    let verification = Verification {
        broke: Vec::new(),
        unsettled: Vec::new(),
    };
    assert!(verification.held());
}
