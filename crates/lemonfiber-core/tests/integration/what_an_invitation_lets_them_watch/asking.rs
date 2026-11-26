//! What an invited member may ask for, and what travels back.

use super::{
    a_stack_asking, a_stack_where_requests_arrive_unseen, ana_requesting, answering, driving,
    held_first, held_to, made_an_account, offering, requesting,
};
use lemonfiber_core::app::{Allowance, Outcome};
use lemonfiber_core::asking::Policy;
use lemonfiber_core::model::Linked;
use lemonfiber_core::ports::service::{Requesting, Unrated};
use lemonfiber_testing::requests;

/// The gap the setting exists to close: what a narrowed member may ask for is held to
/// the same decision as what they may watch.
///
/// The request service has no notion of a content rating, so there is no limit to
/// mirror. What it does have is the difference between a request that lands in the
/// library unseen and one an adult sees first, and the approval bits come off — leaving
/// everything else the account was given exactly as it was.
#[tokio::test]
async fn what_a_narrowed_member_may_ask_for_is_held_to_the_same_decision() {
    let (sent, made) = driving(
        "requests-held",
        a_stack_where_requests_arrive_unseen(),
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(12),
            unrated: None,
        },
    )
    .await;

    assert!(made.is_some(), "the invitation itself was refused");
    assert_eq!(
        held_first(&sent),
        vec!["4".to_owned()],
        "what a narrowed member may ask for was left arriving unseen"
    );
    assert_eq!(
        held_to(made.as_ref()),
        Some(Policy::EverythingWaits),
        "the invitation did not say what the member is now held to when they ask"
    );
}

/// A rehearsal says what the write would do: the request service would hold them, and
/// what they ask for would wait. Nothing is sent to either service.
#[tokio::test]
async fn a_rehearsal_says_what_they_would_be_held_to_when_they_ask() {
    let (sent, made) = answering(
        "requests-rehearsed",
        a_stack_where_requests_arrive_unseen(),
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(12),
            unrated: None,
        },
        false,
    )
    .await;

    let held = held_to(made.as_ref());
    assert_eq!(requesting(made), Some(Linked::Made));
    assert_eq!(held, Some(Policy::EverythingWaits));
    assert!(!made_an_account(&sent), "a rehearsal made an account");
    let service = requests::intake("answers").trim_end_matches("answers");
    assert!(
        sent.iter().all(|one| !one.url.contains(service)),
        "a rehearsal asked the request service: {sent:?}"
    );
}

/// An offer that narrows nobody leaves what they may ask for alone.
///
/// The same rule the allowance is held to, one service along: an offer that says nothing
/// about access must not quietly take a permission off somebody's account.
#[tokio::test]
async fn an_offer_that_narrows_nobody_leaves_what_they_may_ask_for_alone() {
    let (sent, made) = driving(
        "requests-untouched",
        a_stack_where_requests_arrive_unseen(),
        Allowance::default(),
    )
    .await;

    assert!(made.is_some(), "the invitation itself was refused");
    assert!(
        held_first(&sent).is_empty(),
        "an offer that narrowed nobody changed what they may ask for"
    );
}

/// What was written travels back on the answer that wrote it, in the certificates this
/// household's own media server names.
///
/// A member held to a rating who cannot find half the library is either this working or
/// a defect, and an operator with nothing on record cannot tell which.
#[tokio::test]
async fn what_was_applied_travels_back_with_the_invitation() {
    let (_, made) = offering(
        "applied",
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(12),
            unrated: None,
        },
    )
    .await;

    // Flattened rather than unwrapped: an invitation that said nothing about access
    // has to fail the assertions below rather than end the run before them.
    let (limit, unrated, filtering) = match made {
        Some(Outcome::Invitation(invitation)) => {
            invitation
                .applied
                .map_or_else(<(String, Unrated, String)>::default, |applied| {
                    (
                        applied.limit.unwrap_or_default(),
                        applied.unrated,
                        applied.filtering,
                    )
                })
        }
        _ => <(String, Unrated, String)>::default(),
    };

    assert!(
        limit.contains("12A"),
        "the limit was not said in this household's own certificates: {limit}"
    );
    assert_eq!(
        unrated,
        Unrated::HeldBack,
        "unrated content was not held back: {limit}"
    );
    assert!(
        filtering.contains("not a security boundary"),
        "the answer did not say what a limit here is not: {filtering}"
    );
}

/// A limit set on somebody whose requests already wait writes nothing there.
///
/// The narrowest thing the request service can be told is already true of them, and a
/// write that said it again would be a change to an account nobody asked to change.
#[tokio::test]
async fn a_member_whose_requests_already_wait_is_left_as_they_are() {
    let (sent, made) = driving(
        "already-waiting",
        a_stack_asking(ana_requesting(false), requests::taken()),
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(12),
            unrated: None,
        },
    )
    .await;

    assert_eq!(
        requesting(made),
        Some(lemonfiber_core::model::Linked::Made),
        "somebody already held was not reported as held"
    );
    assert!(
        held_first(&sent).is_empty(),
        "an account that already waits was written to anyway"
    );
}

/// A request service that will not take the write still leaves the invitation standing.
///
/// The account on the media server is what an invitation *is*. What could not be held
/// is worth a line on the answer; it is not worth the account.
#[tokio::test]
async fn a_request_service_that_will_not_hold_still_leaves_the_invitation() {
    let (_, made) = driving(
        "will-not-hold",
        a_stack_asking(ana_requesting(true), requests::unavailable()),
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(12),
            unrated: None,
        },
    )
    .await;

    assert_eq!(
        held_to(made.as_ref()),
        None,
        "a member the request service would not hold was said to be held"
    );
    assert_eq!(
        requesting(made),
        Some(lemonfiber_core::model::Linked::NotYet),
        "a write the request service refused was reported as made"
    );
}

/// A request service that will not answer at all is said, and the account stands.
#[tokio::test]
async fn a_request_service_that_will_not_answer_is_said_rather_than_guessed() {
    let (_, made) = driving(
        "will-not-answer",
        a_stack_asking(requests::unavailable(), requests::unavailable()),
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(12),
            unrated: None,
        },
    )
    .await;

    assert_eq!(
        requesting(made),
        Some(lemonfiber_core::model::Linked::NotYet),
        "a service that would not answer was reported as having held somebody"
    );
}

/// A member the request service has never heard of has nothing to hold yet: the next run
/// makes the account and holds it then.
#[tokio::test]
async fn a_member_the_request_service_has_not_heard_of_is_not_tried_yet() {
    let (sent, made) = driving(
        "never-heard-of",
        a_stack_asking(requests::answered(&None::<Requesting>), requests::taken()),
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(12),
            unrated: None,
        },
    )
    .await;

    assert_eq!(
        requesting(made),
        Some(lemonfiber_core::model::Linked::NotTried),
        "a member the request service does not hold was reported as held"
    );
    assert!(held_first(&sent).is_empty(), "{sent:?}");
}
