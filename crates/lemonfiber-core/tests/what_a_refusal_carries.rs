//! What becomes of the reason a refusal carried, driven through the dispatcher.
//!
//! A seam of its own, apart from what a household may ask for: that one is settings and
//! quotas, and this is one message to one person and the record that stops a second.
//! Driven from here as well as in-crate because the app layer is compiled twice, and a
//! branch driven from only one of those is counted as never run in the other.
//!
//! **What is asserted is what the transport was handed**, not the line the operator was
//! shown. Those are different claims, and the one that matters to somebody who asked for
//! something is the first.

use lemonfiber_core::app::{dispatch, Answer as Ruling, Command, Ctx, Decision, Outcome};
use lemonfiber_core::config::{Reaching, REACH_HOUSEHOLD_KEY};
use lemonfiber_core::ports::http::{Method, Request};
use lemonfiber_fixtures::http::Answer;

mod common;

use common::household::{answering, context, reaching, refusing, table, with, HALF_REACHED_AT};

/// The reason reaches the person who asked, where they left an address for it.
///
/// **The whole of what travels is the reason.** The request service already tells them
/// their request was declined and this must not say so again, so what goes is the word
/// `Why` and the operator's own sentence — no name of this product, no address to open,
/// nothing to sign in to. Read off what the transport was actually handed rather than off
/// the line the operator was shown, because those are different claims.
#[tokio::test]
async fn the_reason_reaches_the_person_who_asked_and_carries_nothing_else() {
    let transport = table(Vec::new());
    let said = decided(&reaching("told", &transport, Reaching::default())).await;

    assert!(
        said.contains("told why, on Pushover and Pushbullet"),
        "{said}"
    );
    assert!(!said.contains("yours to pass on"), "{said}");

    let sent: Vec<Request> = transport
        .requests()
        .into_iter()
        .filter(|asked| asked.url.contains("pushover.net"))
        .collect();
    assert_eq!(sent.len(), 1, "{sent:?}");
    let carried = sent
        .first()
        .and_then(|asked| asked.body.clone())
        .unwrap_or_default();
    assert!(
        carried.contains(r#""message":"we already have it dubbed""#),
        "{carried}"
    );
    assert!(carried.contains(r#""title":"Why""#), "{carried}");
    assert!(carried.contains(r#""user":"the-user-key""#), "{carried}");
    for absent in ["lemonfiber", "declined", "http://", "Alex"] {
        assert!(!carried.contains(absent), "{absent} travelled: {carried}");
    }
}

/// An agent the member switched off is not somewhere a message may go.
#[tokio::test]
async fn an_agent_the_member_switched_off_is_left_alone() {
    let transport = table(vec![(
        None,
        "/user/4/settings/notifications",
        Answer::reply(200, HALF_REACHED_AT),
    )]);
    let said = decided(&context("half", &transport)).await;

    assert!(said.contains("told why, on Pushover"), "{said}");
    assert!(
        !transport
            .requests()
            .iter()
            .any(|asked| asked.url.contains("pushbullet.com")),
        "an agent the member switched off was written to anyway"
    );
}

/// The words are written down as carried, so nothing can carry them a second time.
#[tokio::test]
async fn what_was_carried_is_written_down_beside_the_reason() {
    let carried = decided(&answering("told-once")).await;
    assert!(carried.contains("told why"), "{carried}");

    let read = dispatch(Command::Household { member: None }, &answering("told-once"))
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json())
        .unwrap_or_default();

    assert!(
        read.contains(r#""told":{"to":["Pushover","Pushbullet"]"#),
        "nothing records that the words went, so they could go again: {read}"
    );
}

/// An operator who switched this off is told so, and nothing leaves the machine.
#[tokio::test]
async fn a_household_this_machine_may_not_reach_is_said_rather_than_reached() {
    let transport = table(Vec::new());
    let said = decided(&reaching(
        "not-told",
        &transport,
        Reaching::without(REACH_HOUSEHOLD_KEY),
    ))
    .await;

    assert!(said.contains(REACH_HOUSEHOLD_KEY), "{said}");
    assert!(said.contains("yours to pass on"), "{said}");
    assert!(
        !transport
            .requests()
            .iter()
            .any(|asked| asked.url.contains("pushover.net")),
        "a switched-off request went anyway"
    );
}

/// A member with no address of these two kinds is an absence, not a failure.
#[tokio::test]
async fn a_member_with_nowhere_to_reach_them_is_not_a_failure() {
    let said = decided(&with(
        "no-address",
        vec![(
            None,
            "/user/4/settings/notifications",
            Answer::reply(200, "{}"),
        )],
    ))
    .await;

    assert!(said.contains("no address"), "{said}");
    assert!(said.contains("yours to pass on"), "{said}");
    assert!(said.contains("we already have it dubbed"), "{said}");
}

/// A service that would not say where they are reached leaves the words behind, and
/// says which of the two things happened.
#[tokio::test]
async fn where_they_are_reached_that_cannot_be_read_is_said_as_that() {
    let said = decided(&refusing(
        "unreadable",
        Method::Get,
        "/user/4/settings/notifications",
    ))
    .await;

    assert!(said.contains("could not be read"), "{said}");
    assert!(said.contains("yours to pass on"), "{said}");
}

/// Every address refusing names them all and leaves the words with the operator.
#[tokio::test]
async fn every_address_refusing_names_them_and_keeps_the_words_here() {
    let said = decided(&with(
        "refused",
        vec![
            (None, "pushover.net", Answer::reply(500, "no")),
            (None, "pushbullet.com", Answer::reply(500, "no")),
        ],
    ))
    .await;

    assert!(
        said.contains("Pushover and Pushbullet would not take it"),
        "{said}"
    );
    assert!(said.contains("yours to pass on"), "{said}");
}

/// One address taking it and one refusing is told once, and said as told once.
#[tokio::test]
async fn one_address_taking_it_and_one_refusing_is_told_once() {
    let said = decided(&with(
        "mixed",
        vec![(None, "pushbullet.com", Answer::reply(500, "no"))],
    ))
    .await;

    assert!(said.contains("told why, on Pushover"), "{said}");
    assert!(said.contains("Pushbullet would not take it"), "{said}");
    assert!(said.contains("once rather than twice"), "{said}");
}

/// A rehearsal decides nothing and tells nobody.
#[tokio::test]
async fn a_rehearsal_tells_nobody() {
    let transport = table(Vec::new());
    let mut ctx = reaching("rehearsed", &transport, Reaching::default());
    ctx.dry_run = true;
    let said = decided(&ctx).await;

    assert!(said.contains("nothing was sent or decided"), "{said}");
    assert!(
        !transport
            .requests()
            .iter()
            .any(|asked| asked.url.contains("pushover.net")),
        "a rehearsal told somebody"
    );
}

/// An approval carries no reason, writes nothing down and tells nobody.
#[tokio::test]
async fn an_approval_tells_nobody_because_there_is_nothing_to_tell() {
    let transport = table(Vec::new());
    let ctx = reaching("approved", &transport, Reaching::default());
    let said = dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::LetThrough,
        }),
        &ctx,
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default();

    assert!(said.contains("approved"), "{said}");
    assert!(!said.contains("told why"), "{said}");
    assert!(
        !transport
            .requests()
            .iter()
            .any(|asked| asked.url.contains("pushover.net")),
        "an approval told somebody why"
    );
}

/// One request turned down with a reason, as the answer an operator reads back.
async fn decided(ctx: &Ctx) -> String {
    dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::TurnedDown {
                reason: "we already have it dubbed".to_owned(),
            },
        }),
        ctx,
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default()
}
