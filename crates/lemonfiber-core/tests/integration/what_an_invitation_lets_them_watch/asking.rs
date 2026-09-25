//! What an invited member may ask for, and what travels back.

use super::{
    a_stack_where_requests_arrive_unseen, driving, offering, requesting, APPROVES_OWN, AS_IT_OPENS,
    CERTIFICATES, LIBRARIES, NEW_ACCOUNT, ONE_ACCOUNT, PERMISSIONS, POLICY, RATINGS,
};
use lemonfiber_core::app::{Allowance, Outcome};
use lemonfiber_core::ports::service::Unrated;
use lemonfiber_fixtures::http::{Answer, Fake};
use std::sync::Arc;

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
    let held: Vec<serde_json::Value> = sent
        .iter()
        .filter(|request| request.url.contains(PERMISSIONS))
        .filter_map(|request| request.body.as_deref())
        .filter_map(|body| serde_json::from_str(body).ok())
        .collect();

    assert_eq!(
        held,
        vec![serde_json::json!({ "permissions": 32 })],
        "what a narrowed member may ask for was left arriving unseen"
    );
}

/// An offer that narrows nobody leaves what they may ask for alone.
///
/// The same rule the policy is held to, one service along: an offer that says nothing
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
        !sent.iter().any(|request| request.url.contains(PERMISSIONS)),
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
        AS_IT_OPENS,
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

/// The account of somebody whose requests already wait for an adult.
///
/// `32` is `REQUEST` and nothing else: they may ask, and what they ask for is seen
/// first. There is nothing here to take off.
const ALREADY_WAITING: &str = r#"{"id":4,"permissions":32}"#;

/// A stack whose request service holds an account whose requests already wait.
fn a_stack_where_requests_already_wait() -> Arc<Fake> {
    a_stack_asking(
        Answer::reply(200, ALREADY_WAITING),
        Answer::reply(200, ALREADY_WAITING),
    )
}

/// A stack where the request service holds the account and refuses the write.
fn a_stack_that_will_not_hold() -> Arc<Fake> {
    a_stack_asking(Answer::reply(200, APPROVES_OWN), Answer::reply(500, ""))
}

/// The same stack, with what the request service says about a member and what it does
/// with the write chosen.
fn a_stack_asking(member: Answer, written: Answer) -> Arc<Fake> {
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    Fake::by_path_in_turn(vec![
        ("/Users/AuthenticateByName", vec![signed_in]),
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        ("/user/import-from-jellyfin", vec![Answer::reply(201, "{}")]),
        (PERMISSIONS, vec![Answer::reply(200, APPROVES_OWN), written]),
        ("/user/jellyfin/", vec![member]),
        (RATINGS, vec![Answer::reply(200, CERTIFICATES)]),
        (
            "/System/ActivityLog",
            vec![Answer::reply(200, r#"{"Items":[]}"#)],
        ),
        ("/Library/MediaFolders", vec![Answer::reply(200, LIBRARIES)]),
        (POLICY, vec![Answer::reply(204, "")]),
        (
            NEW_ACCOUNT,
            vec![Answer::reply(
                200,
                r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
            )],
        ),
        (ONE_ACCOUNT, vec![Answer::reply(200, AS_IT_OPENS)]),
        ("/Users", vec![Answer::reply(200, "[]")]),
    ])
}

/// A limit set on somebody whose requests already wait writes nothing there.
///
/// The narrowest thing the request service can be told is already true of them, and a
/// write that said it again would be a change to an account nobody asked to change.
#[tokio::test]
async fn a_member_whose_requests_already_wait_is_left_as_they_are() {
    let (sent, made) = driving(
        "already-waiting",
        a_stack_where_requests_already_wait(),
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
        !sent
            .iter()
            .any(|request| request.url.contains(PERMISSIONS) && request.body.is_some()),
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
        a_stack_that_will_not_hold(),
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
        "a write the request service refused was reported as made"
    );
}

/// A request service that will not answer at all is said, and the account stands.
#[tokio::test]
async fn a_request_service_that_will_not_answer_is_said_rather_than_guessed() {
    let (_, made) = driving(
        "will-not-answer",
        a_stack_asking(Answer::reply(500, ""), Answer::reply(500, "")),
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
