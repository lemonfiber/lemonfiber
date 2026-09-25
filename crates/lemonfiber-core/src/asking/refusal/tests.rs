use super::{
    never_asked_here, no_limit_named, no_reason_given, nobody_called, nothing_agreed,
    nothing_to_decide, sooner_than_the_reminder, unreachable, NEVER_HERE, NOBODY, NOTHING_AGREED,
    NOT_WAITING, NO_LIMIT, NO_REASON, TOO_SOON, UNREACHABLE,
};
use crate::error::{Amiss, Severity};

/// A service that would not answer is said as nothing having changed.
#[test]
fn a_service_that_would_not_answer_says_nothing_changed() {
    let problem = unreachable("the policy was not set");

    assert_eq!(problem.code, UNREACHABLE);
    assert!(problem.summary.contains("the policy was not set"));
    assert!(problem.meaning.contains("still has"), "{problem:?}");
}

/// The policy that is a limit refuses to be chosen without one.
#[test]
fn the_policy_that_is_a_limit_refuses_to_be_chosen_without_one() {
    let problem = no_limit_named();

    assert_eq!(problem.code, NO_LIMIT);
    assert_eq!(problem.amiss, Amiss::Asking);
    assert!(problem.remedies.first().is_some_and(|remedy| {
        remedy.action.contains("how many") && remedy.action.contains("how long")
    }));
}

/// A request already ruled on is named rather than decided a second time.
#[test]
fn a_request_already_ruled_on_is_named_rather_than_decided_again() {
    let problem = nothing_to_decide(42);

    assert_eq!(problem.code, NOT_WAITING);
    assert_eq!(problem.amiss, Amiss::Naming);
    assert!(problem.summary.contains("42"), "{problem:?}");
}

/// A blank reason is the silent decline arriving through the field meant to stop it.
#[test]
fn a_blank_reason_is_refused_as_the_silence_it_would_be() {
    let problem = no_reason_given();

    assert_eq!(problem.code, NO_REASON);
    assert_eq!(problem.amiss, Amiss::Asking);
    assert!(problem.meaning.contains("ignored"), "{problem:?}");
}

/// Nobody by that name is refused with the household named beside it.
#[test]
fn nobody_by_that_name_is_refused_with_the_household_named() {
    let problem = nobody_called("sam", &["ana".to_owned(), "bea".to_owned()]);

    assert_eq!(problem.code, NOBODY);
    assert_eq!(problem.amiss, Amiss::Naming);
    assert!(problem.summary.contains("sam"), "{problem:?}");
    let there = problem
        .remedies
        .first()
        .and_then(|remedy| remedy.detail.clone())
        .unwrap_or_default();
    assert!(there.contains("ana") && there.contains("bea"), "{there}");
}

/// Somebody who has never signed in is an invitation unused, not a fault.
///
/// It says what holds them in the meantime, because an operator told only that
/// nothing happened would not know whether they are limited or not.
#[test]
fn somebody_who_has_never_signed_in_is_not_a_fault() {
    let problem = never_asked_here("ana");

    assert_eq!(problem.code, NEVER_HERE);
    assert_eq!(problem.severity, Severity::Warning);
    assert!(problem.summary.contains("ana"), "{problem:?}");
    assert!(problem.meaning.contains("in the meantime"), "{problem:?}");
}

/// A household that never named a period is asked for one rather than given one.
#[test]
fn a_household_that_named_no_period_is_asked_for_one() {
    let problem = nothing_agreed();

    assert_eq!(problem.code, NOTHING_AGREED);
    assert_eq!(problem.amiss, Amiss::Asking);
    assert!(
        problem.meaning.contains("nobody agreed to close"),
        "{problem:?}"
    );
    assert!(
        problem
            .remedies
            .first()
            .and_then(|remedy| remedy.detail.clone())
            .is_some_and(|detail| detail.contains("--after")),
        "the remedy does not say how to name one: {problem:?}"
    );
}

/// A period sooner than the reminder is refused, and refused with the reminder's
/// own figure beside it.
#[test]
fn a_period_sooner_than_the_reminder_is_refused_with_the_reminder_named() {
    let problem = sooner_than_the_reminder(3);

    assert_eq!(problem.code, TOO_SOON);
    assert_eq!(problem.amiss, Amiss::Asking);
    assert!(problem.summary.contains('3'), "{problem:?}");
    assert!(
        problem
            .remedies
            .first()
            .and_then(|remedy| remedy.detail.clone())
            .is_some_and(|detail| detail.contains("7 days")),
        "the refusal does not say what the reminder's own period is: {problem:?}"
    );
}
