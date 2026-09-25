//! A wait that says what it is waiting for.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the start
//! beside it is: the whole path is `async`, and an async path exercised only
//! in-crate has its coverage counted from the copy that never ran.
//!
//! The budget is elapsed in virtual time. A real three-minute wait is not a test
//! anybody would run, and a shortened one would prove the wait speaks at a length
//! nobody ever waits — the number that matters here is the one an operator meets.

use std::sync::Arc;
use std::time::Duration;

use lemonfiber_core::app::{dispatch, started, Command, Ctx};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::Narrator;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::heard::Heard;
use lemonfiber_fixtures::ports::Following;
use lemonfiber_fixtures::support::Reporting;

/// Everything the `library` form declares.
const LIBRARY: [&str; 5] = [
    "jellyfin",
    "seerr",
    "calibre-web-automated",
    "audiobookshelf",
    "navidrome",
];

/// The budget a start is given, which is what these waits elapse.
const PATIENCE: Duration = Duration::from_secs(180);

/// A context whose stack answers this way, waiting the budget a real start waits.
fn ctx(health: Health) -> Ctx {
    lemonfiber_testing::a_context()
        .engine(Arc::new(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            health,
        )))
        .clock(Following::started())
        .filesystem(Files::empty())
        // Faked so a start's port pre-flight never reaches a real daemon: what else is
        // running on the machine a test happens to run on is not a fact the test is about.
        .images(lemonfiber_fixtures::pulled::Pulled::holding(Vec::new()))
        .settings(Settings {
            protocols: Protocols::both(),
            ..Settings::default()
        })
        .build()
        .with_patience(PATIENCE)
}

/// The forms an operator names.
fn named(forms: &[&str]) -> Vec<String> {
    forms.iter().map(|form| (*form).to_owned()).collect()
}

/// Start the `library` form and hand back everything the wait said.
async fn starting(health: Health) -> (Vec<String>, bool) {
    let heard = Arc::new(Heard::default());
    let ctx = ctx(health).with_narrator(Arc::clone(&heard) as Arc<dyn Narrator>);
    let outcome = started(&ctx, &named(&["library"]), &[], Some(0)).await;
    (heard.said(), outcome.is_ok())
}

/// The requirement: a wait long enough to read as a hang says what it is waiting
/// for, and goes on saying it.
///
/// This is the test that fails when the wait goes quiet again. Every assertion
/// below is about the same three minutes of silence the operator used to meet: that
/// something arrives at all, that it keeps arriving, and that what arrives names the
/// services rather than merely proving the process is alive.
#[tokio::test(start_paused = true)]
async fn a_wait_says_what_it_is_waiting_for_while_it_waits() {
    let (said, settled) = starting(Health::Starting).await;

    assert!(
        !settled,
        "the stack never settled, so the start was refused"
    );
    assert!(
        said.len() >= 30,
        "three minutes of waiting, spoken for throughout: {} lines",
        said.len()
    );
    let outstanding: Vec<&String> = said
        .iter()
        .filter(|line| LIBRARY.iter().all(|service| line.contains(service)))
        .collect();
    assert_eq!(
        outstanding.len(),
        said.len(),
        "every line names what it is waiting for: {said:?}"
    );
}

/// The first line arrives while the operator is still watching, rather than at the
/// end of a budget they have already given up on — and this is the whole of it, as
/// the operator reads it: the services the stack declares, in its own order, and how
/// far into the budget the wait has got.
#[tokio::test(start_paused = true)]
async fn the_first_line_arrives_seconds_in_rather_than_minutes_in() {
    let (said, _) = starting(Health::Starting).await;

    assert_eq!(
        said.first().map(String::as_str),
        Some(
            "Still starting: audiobookshelf, calibre-web-automated, jellyfin, navidrome, \
             seerr — 5 seconds so far, of 180."
        )
    );
}

/// Each line says something the one above it did not, so a wait that has not
/// changed is still worth reading — which is the difference between progress and a
/// screen reprinting itself.
#[tokio::test(start_paused = true)]
async fn no_two_lines_of_one_wait_are_the_same() {
    let (said, _) = starting(Health::Starting).await;

    let mut seen = said.clone();
    seen.sort_unstable();
    seen.dedup();
    // Asked of a wait that spoke, because "no two are the same" is true of nothing
    // at all — and nothing at all is the failure the file is about.
    assert!(!said.is_empty(), "the wait spoke");
    assert_eq!(seen.len(), said.len(), "all different: {said:?}");
}

/// A start that is over before anybody could doubt it says nothing at all. Remarking
/// on a two-second wait is what teaches an operator that these lines are noise,
/// before the day one of them matters.
#[tokio::test(start_paused = true)]
async fn a_start_that_settles_at_once_says_nothing() {
    let (said, settled) = starting(Health::Healthy).await;

    assert!(settled, "the stack came up");
    assert_eq!(said, Vec::<String>::new());
}

/// A rehearsal waits for nothing, so it has nothing to say about waiting. It stops
/// before the single irreversible step, and the wait is on the far side of it.
///
/// It is not silent, and that is deliberate elsewhere: a rehearsal states what the
/// real run would take away, because what it would cost is the one question the flag
/// exists to answer. So the claim here is about the wait rather than about the
/// narrator being empty — asserting emptiness would make this test fail the day any
/// other sentence is added, whatever it said.
#[tokio::test(start_paused = true)]
async fn a_rehearsal_waits_for_nothing_and_says_nothing_about_waiting() {
    let heard = Arc::new(Heard::default());
    let ctx = ctx(Health::Starting)
        .with_narrator(Arc::clone(&heard) as Arc<dyn Narrator>)
        .rehearsing();

    let rehearsed = dispatch(
        Command::Up {
            forms: named(&["library"]),
        },
        &ctx,
    )
    .await;

    assert!(rehearsed.is_ok(), "a rehearsal reports what would run");
    let said = heard.said();
    assert!(
        said.iter()
            .all(|line| line.starts_with("this takes services")),
        "a rehearsal narrated a wait it never did: {said:?}"
    );
}

/// A run nobody is listening to waits and refuses exactly as one being listened to
/// does. The narration is something a surface reads, never something the wait
/// depends on having somewhere to go.
#[tokio::test(start_paused = true)]
async fn a_wait_nobody_is_listening_to_ends_the_same_way() {
    let ctx = ctx(Health::Starting);

    let refused = started(&ctx, &named(&["library"]), &[], Some(0)).await;

    assert!(refused.is_err(), "the stack never settled");
}
