//! What becomes of a request nobody ruled on, driven through the dispatcher.
//!
//! A seam of its own, apart from what a household may ask for and apart from what a
//! refusal carries: those are an operator answering, and this is nobody answering. Driven
//! from here as well as in-crate because the app layer is compiled twice, and a branch
//! driven from only one of those is counted as never run in the other.
//!
//! **What is asserted is what the request service and the transport were handed**, and
//! what the household reads back afterwards — not the line the operator was shown. Those
//! are different claims, and for somebody who asked for something the first two are the
//! ones that happened to them.

use std::time::Duration;

use lemonfiber_core::app::{dispatch, Arranged, Command, Ctx, Outcome};
use lemonfiber_core::ports::http::Request;
use lemonfiber_fixtures::http::Fake;

mod common;

use common::household::{answering, context, table, watched};

/// What one run of the arrangement, or of the clock, came to say.
async fn said(ctx: &Ctx, arranged: Arranged) -> String {
    match dispatch(Command::Expiring(arranged), ctx).await {
        Ok(Outcome::Household(report)) => report.findings.join("\n"),
        Ok(other) => format!("the wrong outcome: {other:?}"),
        Err(problem) => format!("refused: {} {}", problem.code, problem.summary),
    }
}

/// Naming a period records it and closes nothing, and says both.
///
/// **The two acts are apart on purpose.** Recording is what lets the household be told
/// in advance, on every reading between the arrangement and the first thing it reaches;
/// and a run that closed something on the way to recording would have closed it before
/// anybody had been told anything.
#[tokio::test]
async fn naming_a_period_records_it_and_closes_nothing() {
    let (ctx, transport) = watched("arranged");

    let arranged = said(&ctx, Arranged::After(30)).await;

    assert!(arranged.contains("30 days"), "{arranged}");
    assert!(
        arranged.contains("starts nothing by itself"),
        "the arrangement implied a background this product has not got: {arranged}"
    );
    assert!(
        !transport
            .requests()
            .iter()
            .any(|asked| asked.url.contains("/request/7/decline")),
        "arranging a period closed something"
    );
}

/// A period sooner than the reminder is refused, and arranges nothing.
#[tokio::test]
async fn a_period_sooner_than_the_reminder_is_refused() {
    let refused = said(&answering("too-soon-here"), Arranged::After(3)).await;

    assert!(refused.starts_with("refused: QUOTA-9"), "{refused}");
}

/// A run against an arrangement nobody made is refused rather than given one.
///
/// The whole of what keeps this from being a policy nobody consented to: there is no
/// period this program will choose on a household's behalf.
#[tokio::test]
async fn a_run_against_no_arrangement_is_refused() {
    let refused = said(&answering("unarranged-here"), Arranged::AsAgreed).await;

    assert!(refused.starts_with("refused: QUOTA-8"), "{refused}");
    assert!(
        refused.contains("no period has been agreed to"),
        "{refused}"
    );
}

/// Withdrawing it puts the household back to closing nothing.
#[tokio::test]
async fn withdrawing_it_puts_the_household_back_to_closing_nothing() {
    let ctx = answering("withdrawn-here");
    assert!(said(&ctx, Arranged::After(30)).await.contains("30 days"));

    let withdrawn = said(&ctx, Arranged::Never).await;

    assert!(
        withdrawn.contains("waits until somebody rules on it"),
        "{withdrawn}"
    );
    let refused = said(&ctx, Arranged::AsAgreed).await;
    assert!(
        refused.starts_with("refused: QUOTA-8"),
        "a withdrawn arrangement was run on anyway: {refused}"
    );
}

/// Between the arrangement and the first thing it closes, the household is told.
///
/// **Both readers, in the words each of them needs.** The operator is told the period,
/// what closes them and that nothing runs it; the member is told their wait can end
/// unanswered and that they will hear which — and is told no date, because the period is
/// the operator's to change and a date said to a member is one this could not keep.
#[tokio::test]
async fn the_household_is_told_before_anything_reaches_it() {
    let ctx = answering("told-in-advance");
    assert!(said(&ctx, Arranged::After(30)).await.contains("30 days"));

    let read = dispatch(Command::Household { member: None }, &ctx)
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json())
        .unwrap_or_default();

    assert!(
        read.contains("closed after 30 days while `lemonfiber household expiring` is running"),
        "the operator is not told what closes them: {read}"
    );
    assert!(
        read.contains("nothing runs it for you"),
        "the operator is left believing this happens by itself: {read}"
    );
    assert!(
        read.contains("closed for having waited too long"),
        "the member is still promised nothing ends the wait: {read}"
    );
    assert!(
        !read.contains("Nothing expires it"),
        "the member is told two opposite things: {read}"
    );
}

/// The clock closes what ran out, tells whoever asked why, and leaves it on the record.
///
/// **Interrupted where a terminal interrupts it**, because the run holds until the
/// arrangement changes and what is being asserted is what one wake of it did. That is the
/// honest shape of the claim as well as the practical one: an operator stops this with
/// their own interruption, and everything it did before that is in the record rather than
/// in a summary it never got to print.
#[tokio::test(start_paused = true)]
async fn the_clock_closes_what_ran_out_and_leaves_it_all_on_the_record() {
    let transport = table(Vec::new());
    let ctx = context("closed", &transport);
    assert!(said(&ctx, Arranged::After(30)).await.contains("30 days"));

    let _stopped = tokio::time::timeout(
        Duration::from_millis(200),
        dispatch(Command::Expiring(Arranged::AsAgreed), &ctx),
    )
    .await;

    assert!(
        closed(&transport),
        "nothing was closed at the request service"
    );
    let carried = sent(&transport, "pushover.net");
    assert!(carried.contains("30 days"), "{carried}");
    assert!(carried.contains(r#""title":"Why""#), "{carried}");
    // The request service sends the decline itself, so what leaves here is why and
    // nothing else — a second message saying it was declined is the duplicate this
    // product refuses to send.
    for absent in ["declined", "lemonfiber", "http://", "Alex"] {
        assert!(!carried.contains(absent), "{absent} travelled: {carried}");
    }

    let read = dispatch(Command::Household { member: None }, &ctx)
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json())
        .unwrap_or_default();
    assert!(
        read.contains(r#""expired":true"#),
        "what ran out is not readable as having run out: {read}"
    );
    assert!(
        read.contains(r#""told":{"to":["Pushover","Pushbullet"]"#),
        "who was told is not on the record: {read}"
    );
}

/// Nobody is told twice, however many times the clock wakes.
///
/// Held by a record rather than by care: the words are carried only where the
/// record says they have not been, and the record is written whether anything was reached
/// or not — so a run started twice over the same closure sends nothing the second time.
#[tokio::test(start_paused = true)]
async fn nobody_is_told_twice_however_often_it_runs() {
    let transport = table(Vec::new());
    let ctx = context("told-once-here", &transport);
    assert!(said(&ctx, Arranged::After(30)).await.contains("30 days"));

    for _ in 0..2 {
        let _stopped = tokio::time::timeout(
            Duration::from_millis(200),
            dispatch(Command::Expiring(Arranged::AsAgreed), &ctx),
        )
        .await;
    }

    let told: Vec<Request> = transport
        .requests()
        .into_iter()
        .filter(|asked| asked.url.contains("pushover.net"))
        .collect();
    assert_eq!(told.len(), 1, "the household was told twice: {told:?}");
}

/// Whether the request service was asked to close the one request that ran out.
fn closed(transport: &std::sync::Arc<Fake>) -> bool {
    transport
        .requests()
        .iter()
        .any(|asked| asked.url.contains("/request/7/decline"))
}

/// The body of the first request sent to a host, or nothing where none was.
fn sent(transport: &std::sync::Arc<Fake>, host: &str) -> String {
    transport
        .requests()
        .into_iter()
        .find(|asked| asked.url.contains(host))
        .and_then(|asked| asked.body)
        .unwrap_or_default()
}
