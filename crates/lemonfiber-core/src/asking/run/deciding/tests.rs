use super::{deciding, reason_given, said_of, still_waiting};
use crate::app::command::{Answer, Decision};
use crate::ports::service::HouseholdRequest;
use crate::test_support::a_context;

/// One request as the service records it, at the two statuses that decide its state.
fn asked(id: i64, request_status: u8, media_status: u8) -> HouseholdRequest {
    HouseholdRequest {
        id,
        made: Some("2026-08-17T21:04:09".to_owned()),
        member: "Ana".to_owned(),
        kind: Some(crate::recyclarr::Kind::Radarr),
        item: None,
        request_status,
        media_status,
    }
}

/// A stack that cannot be read rules on nothing, and says so as a stack.
///
/// The manifest is read before the request service is reached at all, so a stack
/// nobody could read is its own answer rather than a service that would not speak.
#[tokio::test]
async fn a_stack_that_cannot_be_read_rules_on_nothing() {
    let ctx = a_context().over(crate::test_support::nowhere()).build();

    let refused = deciding(
        &ctx,
        &Decision {
            request: 7,
            answer: Answer::LetThrough,
        },
    )
    .await;

    assert!(
        refused.is_err(),
        "a stack nobody could read was ruled against"
    );
    assert_ne!(
        refused.err().map(|problem| problem.code),
        Some(crate::error::codes::quota::UNREACHABLE),
        "a stack that would not read was reported as a service that would not answer"
    );
}

/// A request nobody has ruled on is the one that can be ruled on.
#[test]
fn a_request_nobody_has_ruled_on_is_the_one_that_can_be_decided() {
    let held = [asked(7, 1, 1)];

    assert!(still_waiting(&held, 7).is_some());
}

/// One already decided, and one this service does not hold, are both nothing.
#[test]
fn one_already_decided_and_one_it_does_not_hold_are_both_nothing() {
    let held = [asked(7, 2, 5), asked(8, 3, 1)];

    assert!(
        still_waiting(&held, 7).is_none(),
        "approved, and decided again"
    );
    assert!(
        still_waiting(&held, 8).is_none(),
        "declined, and decided again"
    );
    assert!(still_waiting(&held, 99).is_none());
    assert!(still_waiting(&[], 7).is_none());
}

/// An approval carries no reason and needs none.
#[test]
fn an_approval_carries_no_reason_and_needs_none() {
    assert_eq!(reason_given(&Answer::LetThrough).unwrap_or(Some("x")), None);
}

/// A decline's reason is trimmed, and one that says nothing is refused.
#[test]
fn a_reason_that_says_nothing_is_refused() {
    // Bound rather than passed inline: the answer owns the reason, and a temporary
    // built in the call would be gone before what it lent out was read.
    let padded = Answer::TurnedDown {
        reason: "  the disk is nearly full  ".to_owned(),
    };
    let given = reason_given(&padded);
    assert_eq!(given.unwrap_or(None), Some("the disk is nearly full"));

    for blank in ["", "   ", "\t\n"] {
        let empty = Answer::TurnedDown {
            reason: blank.to_owned(),
        };
        let refused = reason_given(&empty);
        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(crate::error::codes::quota::NO_REASON),
            "{blank:?} was accepted"
        );
    }
}

/// A decline says the reason back, and says the service carries none.
///
/// What became of the words is the line underneath and not this one's to claim: a
/// decision that said they were passed on would be reporting something this cannot
/// know until it has tried.
#[test]
fn a_decline_says_the_reason_back_and_that_the_service_holds_none() {
    let said = said_of(&asked(7, 1, 1), Some("we are out of room"));

    assert!(said.contains("Ana"), "{said}");
    assert!(said.contains("we are out of room"), "{said}");
    assert!(said.contains("kept here"), "{said}");
    assert!(!said.contains("pass on"), "{said}");
}

/// An approval says it is being fetched, and says nothing about a reason.
#[test]
fn an_approval_says_it_is_being_fetched() {
    let said = said_of(&asked(7, 1, 1), None);

    assert!(said.contains("Ana"), "{said}");
    assert!(said.contains("approved"), "{said}");
    assert!(!said.contains("pass on"), "{said}");
}
