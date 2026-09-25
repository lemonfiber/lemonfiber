//! Choosing what the household may ask for.

use super::*;

/// A choice for the whole household is written and read back as the household.
#[tokio::test]
async fn a_choice_for_the_household_answers_with_the_household() {
    let report = allowing(
        &a_household("house"),
        &Chosen {
            member: None,
            policy: Some(Policy::WithinALimit),
            quota: Some(FIVE_A_WEEK),
        },
    )
    .await
    .unwrap_or_default();

    let said = report.findings.first().cloned().unwrap_or_default();
    assert!(said.contains("5 requests a week"), "{said}");
    assert!(said.starts_with("the household"), "{said}");
}

/// A choice for one person names them, and leaves the household's own alone.
#[tokio::test]
async fn a_choice_for_one_person_names_them() {
    let report = allowing(
        &a_household("person"),
        &Chosen {
            member: Some("alex".to_owned()),
            policy: Some(Policy::EverythingWaits),
            quota: None,
        },
    )
    .await
    .unwrap_or_default();

    let said = report.findings.first().cloned().unwrap_or_default();
    assert!(said.starts_with("Alex"), "{said}");
    assert!(said.contains("waits for you"), "{said}");
    // Their own limit and not the household's. This fixture's household has none
    // and this member has five a week, so the figure in the line is the whole of
    // what says which of the two was read before anything was written.
    assert!(said.contains("5 requests a week"), "{said}");
}

/// A stack that cannot be read changes nothing, and says so as a stack.
///
/// Both writes start by reading the manifest, and what a stack it could not read
/// is has nothing to do with the request service — so it is reported as itself
/// rather than folded into the refusal about a service that would not answer.
#[tokio::test]
async fn a_stack_that_cannot_be_read_changes_nothing() {
    let ctx = a_context().over(crate::test_support::nowhere()).build();

    let refused = allowing(
        &ctx,
        &Chosen {
            member: None,
            policy: Some(Policy::Trusted),
            quota: None,
        },
    )
    .await;

    assert!(refused.is_err(), "a stack nobody could read was acted on");
    assert_ne!(
        refused.err().map(|problem| problem.code),
        Some(crate::error::codes::quota::UNREACHABLE),
        "a stack that would not read was reported as a service that would not answer"
    );
}

/// Nobody by that name is refused with the household named beside it.
#[tokio::test]
async fn nobody_by_that_name_is_refused() {
    let refused = allowing(
        &a_household("nobody"),
        &Chosen {
            member: Some("sam".to_owned()),
            policy: Some(Policy::Trusted),
            quota: None,
        },
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(crate::error::codes::quota::NOBODY)
    );
}

/// A rehearsal says what it would do and writes nothing.
#[tokio::test]
async fn a_rehearsal_says_what_it_would_do_and_writes_nothing() {
    let rehearsing = a_household("rehearsed").rehearsing();

    let report = allowing(
        &rehearsing,
        &Chosen {
            member: None,
            policy: Some(Policy::Trusted),
            quota: None,
        },
    )
    .await
    .unwrap_or_default();
    let one_person = allowing(
        &rehearsing,
        &Chosen {
            member: Some("alex".to_owned()),
            policy: Some(Policy::Trusted),
            quota: None,
        },
    )
    .await
    .unwrap_or_default();

    for said in [report, one_person] {
        let first = said.findings.first().cloned().unwrap_or_default();
        assert!(first.contains("rehearsed"), "{first}");
    }
}

/// A request nobody is waiting on is named rather than ruled on twice.
#[tokio::test]
async fn a_request_nobody_is_waiting_on_is_named() {
    let refused = deciding(
        &a_household("missing"),
        &Decision {
            request: 99,
            answer: Ruling::LetThrough,
        },
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(crate::error::codes::quota::NOT_WAITING)
    );
}

/// What the service holds before a choice is made.
const fn holding(approves_own: bool, quota: Option<Quota>) -> Asking {
    Asking {
        approves_own,
        quota,
    }
}

/// Naming only a limit leaves the policy where it was.
///
/// A run that said nothing about the policy is not a run that chose one, and a
/// value written for it would be this code deciding on the household's behalf.
#[test]
fn naming_only_a_limit_leaves_the_policy_where_it_was() {
    let held = holding(false, None);

    let wanted = settled(
        &Chosen {
            quota: Some(FIVE_A_WEEK),
            ..Chosen::default()
        },
        &held,
    )
    .unwrap_or(held);

    assert!(!wanted.approves_own, "the policy moved");
    assert_eq!(wanted.quota, Some(FIVE_A_WEEK));
}

/// Naming only a policy leaves the limit where it was.
#[test]
fn naming_only_a_policy_leaves_the_limit_where_it_was() {
    let held = holding(false, Some(FIVE_A_WEEK));

    let wanted = settled(
        &Chosen {
            policy: Some(Policy::WithinALimit),
            ..Chosen::default()
        },
        &held,
    )
    .unwrap_or(held);

    assert!(wanted.approves_own);
    assert_eq!(wanted.quota, Some(FIVE_A_WEEK));
}

/// Trusting everybody lifts the limit rather than leaving one counting unseen.
///
/// A household told nothing limits it while the service goes on counting is two
/// answers to one question, and the one the operator reads is the wrong one.
#[test]
fn trusting_everybody_lifts_the_limit_rather_than_leaving_it_counting() {
    let held = holding(true, Some(FIVE_A_WEEK));

    let wanted = settled(
        &Chosen {
            policy: Some(Policy::Trusted),
            ..Chosen::default()
        },
        &held,
    )
    .unwrap_or(held);

    assert_eq!(wanted.quota, None);
    assert!(wanted.approves_own);
}

/// Living within a limit with none named, and none in force, is refused.
#[test]
fn living_within_a_limit_with_none_anywhere_is_refused() {
    let refused = settled(
        &Chosen {
            policy: Some(Policy::WithinALimit),
            ..Chosen::default()
        },
        &holding(false, None),
    );

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(crate::error::codes::quota::NO_LIMIT)
    );
}

/// The same policy with a limit already in force is not refused.
///
/// Choosing to live within the limit you already have is a request, and refusing
/// it would make the operator retype a number the service already holds.
#[test]
fn the_same_policy_over_a_limit_already_in_force_is_not_refused() {
    let wanted = settled(
        &Chosen {
            policy: Some(Policy::WithinALimit),
            ..Chosen::default()
        },
        &holding(false, Some(FIVE_A_WEEK)),
    );

    assert!(wanted.is_ok(), "{wanted:?}");
}

/// What one member is under is read off their own counts, and off whichever of
/// the two halves carries a limit.
///
/// A household chooses one figure and both halves are written together, so the
/// one that is set is the one read back — and a member with no limit of their own
/// reads as having none rather than as having the household's.
#[test]
fn what_one_member_is_under_is_read_off_their_own_counts() {
    let counted = |limit, days| crate::ports::service::Left {
        limit,
        used: 0,
        days,
    };

    let films = theirs(
        true,
        crate::ports::service::Headroom {
            films: counted(Some(3), Some(7)),
            television: counted(None, None),
        },
    );
    assert_eq!(
        films.quota,
        Some(Quota {
            requests: 3,
            days: 7
        })
    );
    assert!(films.approves_own);

    let television = theirs(
        false,
        crate::ports::service::Headroom {
            films: counted(None, None),
            television: counted(Some(4), Some(30)),
        },
    );
    assert_eq!(
        television.quota,
        Some(Quota {
            requests: 4,
            days: 30
        })
    );
    assert!(!television.approves_own);

    assert_eq!(
        theirs(true, crate::ports::service::Headroom::default()).quota,
        None
    );
}

/// Each arrangement reads back as its own line.
#[test]
fn each_arrangement_reads_back_as_its_own_line() {
    assert_eq!(
        now_reads(&holding(true, Some(FIVE_A_WEEK))),
        "may ask for 5 requests a week without anybody seeing it first"
    );
    assert!(now_reads(&holding(false, Some(FIVE_A_WEEK))).starts_with("waits for you"));
    assert!(now_reads(&holding(true, None)).contains("everything anybody asks for"));
    assert!(now_reads(&holding(false, None)).contains("until you have said yes"));
}
