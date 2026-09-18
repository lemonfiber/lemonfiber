//! What a household member gets once the door has named them.
//!
//! Its own file because it answers a different question from its neighbour. `door`
//! settles **who** is asking; this settles **what** that person may have, and the
//! two have different readers — somebody checking that a wrong password is refused
//! is not the person checking that a member cannot start the stack.
//!
//! The decision itself is proved where it is made, in `entitled`'s own tests, and
//! exactly. What is proved here is that every door reaches it: a read, an action and
//! a step of setup, each of which builds a command by its own route. A decision
//! nothing consults is the failure mode these exist against, and it is one a unit
//! test cannot see.

mod door;

use std::fs;

use door::*;

/// The id the media server files the member under, and the account they sign in to.
const MEMBER: &str = "a7f3";

/// The name that member signs in with. The password beside it is built rather than
/// written, in `door`, for the reason every other one here is.
const WHO: &str = "ana";

/// A surface with a household in it, and the headers a member's own session travels
/// on.
async fn as_a_member(named: &str) -> (axum::Router, Vec<(&'static str, String)>) {
    let (router, _, _) = door_with(
        Some(keeping(named)),
        AHousehold::knowing(MEMBER),
        not_the_token(),
    );
    let answer = asked(
        router.clone(),
        "POST",
        SESSION,
        &from_here(),
        &offering_as(WHO, &hers()),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "the household knows this member and the door did not let them in"
    );

    let opened = session(&answer.body);
    assert!(!opened.is_empty(), "no session was handed out");
    let mut carried = from_here();
    carried.push((TOKEN_HEADER, opened));
    (router, carried)
}

/// The same surface, carrying the token printed at the machine instead.
fn as_the_machine(named: &str) -> (axum::Router, Vec<(&'static str, String)>) {
    let (router, token, _) = door_with(
        Some(keeping(named)),
        AHousehold::knowing(MEMBER),
        not_the_token(),
    );
    let mut carried = from_here();
    carried.push((TOKEN_HEADER, token.as_str().to_owned()));
    (router, carried)
}

/// Refused, and not emptied.
///
/// The distinction is the whole requirement. A report with nothing in it reads as
/// *there is nothing there*, and a member who drew that conclusion about their own
/// household would be wrong about it — so the answer has to be a refusal, and the
/// status has to say so.
#[tokio::test]
async fn a_member_is_refused_a_read_that_is_not_theirs() {
    let (router, carried) = as_a_member("member-refused-read").await;
    let answer = asked(router, "GET", "/api/version", &carried, "").await;

    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert!(
        answer.body.contains("may ask for"),
        "a member was turned away without being told it was theirs to be told: {}",
        answer.body
    );
    let _ = fs::remove_dir_all(a_directory("member-refused-read"));
}

/// Said plainly, where the door's own refusals are deliberately vague.
///
/// Those answer somebody who has proved nothing, and naming what was wrong would
/// help them guess again. This one answers somebody who proved who they are, so
/// there is nothing left to guess.
#[tokio::test]
async fn the_refusal_a_member_gets_is_not_the_one_a_stranger_gets() {
    let (router, carried) = as_a_member("member-two-refusals").await;
    let refused = asked(router.clone(), "GET", "/api/version", &carried, "").await;
    let stranger = asked(router, "GET", "/api/version", &from_here(), "").await;

    assert_eq!(refused.status, stranger.status, "both are forbidden");
    assert_ne!(
        refused.body, stranger.body,
        "a member who is not entitled and somebody carrying nothing were told the \
         same thing, so one of them was told something untrue"
    );
    let _ = fs::remove_dir_all(a_directory("member-two-refusals"));
}

/// The one read that is theirs is let through rather than caught by the same net.
///
/// What it answers with is the core's business and is settled where the core is
/// tested; what matters here is that the gate did not stop it.
#[tokio::test]
async fn a_member_reaches_the_one_read_that_is_theirs() {
    let (router, carried) = as_a_member("member-own-read").await;
    let answer = asked(router, "GET", "/api/requests", &carried, "").await;

    assert_ne!(
        answer.status,
        StatusCode::FORBIDDEN,
        "the read a member opens the app to see was refused: {}",
        answer.body
    );
    let _ = fs::remove_dir_all(a_directory("member-own-read"));
}

/// The actions door reaches the same decision the reads door does.
///
/// A member starting the stack is the kind of thing the app would never offer and a
/// hand-written request would ask for anyway, which is the case the requirement is
/// about: the core refuses it, rather than the app having omitted the button.
#[tokio::test]
async fn a_member_is_refused_an_action_whatever_the_app_would_have_shown() {
    let (router, carried) = as_a_member("member-action").await;
    let answer = asked(router, "POST", "/api/actions/up", &carried, "{}").await;

    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    let _ = fs::remove_dir_all(a_directory("member-action"));
}

/// And so does setup, which builds its commands by a third route of its own.
#[tokio::test]
async fn a_member_is_refused_a_step_of_setup() {
    let (router, carried) = as_a_member("member-setup").await;
    let answer = asked(router, "GET", "/api/setup", &carried, "").await;

    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    let _ = fs::remove_dir_all(a_directory("member-setup"));
}

/// Nothing above narrowed what was already answered.
///
/// The same three doors, reached with the token printed at the machine, and none of
/// them refuses — so the gate turns away the person it was written for and nobody
/// else.
#[tokio::test]
async fn the_machine_is_refused_none_of_the_three() {
    let (router, carried) = as_the_machine("machine-unchanged");

    for (method, path, body) in [
        ("GET", "/api/version", ""),
        ("POST", "/api/actions/up", "{}"),
        ("GET", "/api/setup", ""),
    ] {
        let answer = asked(router.clone(), method, path, &carried, body).await;
        assert_ne!(
            answer.status,
            StatusCode::FORBIDDEN,
            "{path} was refused to the machine: {}",
            answer.body
        );
    }
    let _ = fs::remove_dir_all(a_directory("machine-unchanged"));
}
