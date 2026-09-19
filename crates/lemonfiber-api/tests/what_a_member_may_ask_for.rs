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

/// A member whose household has gone quiet is refused, and told which fact it is.
///
/// Not the silence a stranger gets. They proved who they are and the question that
/// could not be answered is about the media server, so the sentence says that — the
/// thing to fix is the server, not their account, and a refusal that implied
/// otherwise would send somebody to change a password that was never wrong.
#[tokio::test]
async fn a_member_whose_household_went_quiet_is_told_what_could_not_be_checked() {
    let named = "member-unreachable-read";
    let household = AHousehold::knowing(MEMBER);
    let (router, _, _) = door_with(
        Some(keeping(named)),
        Arc::clone(&household),
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
    assert_eq!(answer.status, StatusCode::OK);
    let mut carried = from_here();
    carried.push((TOKEN_HEADER, session(&answer.body)));

    household.go_dark();
    let refused = asked(router, "GET", "/api/requests", &carried, "").await;

    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert!(
        refused.body.contains("media server"),
        "somebody was refused without being told the media server is what could not \
         be asked: {}",
        refused.body
    );
    assert!(
        !refused.body.contains("no token or session"),
        "a member whose household went quiet was answered as somebody carrying \
         nothing: {}",
        refused.body
    );
    let _ = fs::remove_dir_all(a_directory(named));
}

/// The stream is a request like any other, and refuses the same way.
///
/// Its own guard, because it brings its own state and is merged outside the layer
/// that covers the rest — which is exactly the assembly mistake that would leave it
/// open while every other route was closed.
#[tokio::test]
async fn a_member_whose_household_went_quiet_is_refused_the_stream_too() {
    let named = "member-unreachable-stream";
    let household = AHousehold::knowing(MEMBER);
    let (router, _, _) = door_with(
        Some(keeping(named)),
        Arc::clone(&household),
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
    let mut carried = from_here();
    carried.push((TOKEN_HEADER, session(&answer.body)));

    household.go_dark();
    let refused = asked(router, "GET", "/api/events", &carried, "").await;

    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert!(refused.body.contains("media server"), "{}", refused.body);
    let _ = fs::remove_dir_all(a_directory(named));
}

/// The sign-in door stays open to somebody the household could not be asked about.
///
/// The whole reason unconfirmed is not nobody. A member whose media server was
/// restarting can sign in again the moment it answers, rather than being held out
/// by the same silence that turns away a stranger.
#[tokio::test]
async fn the_door_is_still_open_to_somebody_who_could_not_be_checked() {
    let named = "member-unreachable-door";
    let household = AHousehold::knowing(MEMBER);
    let (router, _, _) = door_with(
        Some(keeping(named)),
        Arc::clone(&household),
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
    let mut carried = from_here();
    carried.push((TOKEN_HEADER, session(&answer.body)));

    household.go_dark();
    // Carrying the unconfirmed session at the one path that opens without one. The
    // door answers about the pair offered rather than refusing the session, so a
    // household coming back is all it takes to be let in again.
    let again = asked(
        router,
        "POST",
        SESSION,
        &carried,
        &offering_as(WHO, &hers()),
    )
    .await;

    assert_ne!(
        again.status,
        StatusCode::FORBIDDEN,
        "the sign-in door refused somebody it had not been able to ask about: {}",
        again.body
    );
    let _ = fs::remove_dir_all(a_directory(named));
}

/// A member session on a build that has no household is one nothing can vouch for.
///
/// It cannot arise from signing in — a member proves themselves *to* a household —
/// so it is held directly here. The arm exists because a build can lose its
/// household between minting a session and answering with it, and the safe reading
/// of a session nothing can check is that nobody has been identified.
#[tokio::test]
async fn a_member_session_with_no_household_behind_it_is_unconfirmed() {
    let named = "member-no-household";
    let (router, _, admitting) = door(Some(keeping(named)), not_the_token());
    let Some(opened) = admitting
        .sessions
        .opened(
            &not_the_token(),
            moment(),
            Opened::Member(MEMBER.to_owned()),
        )
        .await
    else {
        unreachable!("a source that answers mints a session")
    };
    let mut carried = from_here();
    carried.push((TOKEN_HEADER, opened.token));

    let refused = asked(router, "GET", "/api/requests", &carried, "").await;

    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert!(refused.body.contains("media server"), "{}", refused.body);
    let _ = fs::remove_dir_all(a_directory(named));
}
