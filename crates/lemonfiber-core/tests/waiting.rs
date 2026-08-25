//! A wait that says what it is waiting for.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the start
//! beside it is: the whole path is `async`, and an async path exercised only
//! in-crate has its coverage counted from the copy that never ran.
//!
//! The budget is elapsed in virtual time. A real three-minute wait is not a test
//! anybody would run, and a shortened one would prove the wait speaks at a length
//! nobody ever waits — the number that matters here is the one an operator meets.

mod common;

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use common::stack::project;
use lemonfiber_core::app::{dispatch, started, Command, Ctx};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::{Clock, Narrator};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};
use tokio::sync::Mutex;

/// Everything the `library` form declares.
const LIBRARY: [&str; 4] = [
    "jellyfin",
    "seerr",
    "calibre-web-automated",
    "audiobookshelf",
];

/// The budget a start is given, which is what these waits elapse.
const PATIENCE: Duration = Duration::from_secs(180);

/// A clock that moves as the test's own time does.
///
/// The wait reads the clock to know how far into its budget it is, and a stopped
/// clock is a wait that never ends. This follows the runtime's own time instead, so
/// a paused test elapses three minutes in the time it takes to poll three hundred
/// and sixty times — the budget as an operator meets it, without the waiting.
struct Following {
    /// The wall-clock moment the test started at, so what is stamped is fixed.
    from: SystemTime,
    /// The runtime moment to measure from.
    since: tokio::time::Instant,
}

impl Following {
    /// Started now, at a fixed wall-clock moment.
    fn started() -> Arc<Self> {
        Arc::new(Self {
            from: SystemTime::UNIX_EPOCH + Duration::from_secs(1_786_968_000),
            since: tokio::time::Instant::now(),
        })
    }
}

impl Clock for Following {
    fn now(&self) -> SystemTime {
        self.from + self.since.elapsed()
    }
}

/// Everything a wait said, kept in the order it said it.
#[derive(Default)]
struct Heard(Mutex<Vec<String>>);

#[async_trait]
impl Narrator for Heard {
    async fn say(&self, said: &str) {
        self.0.lock().await.push(said.to_owned());
    }
}

impl Heard {
    /// What it has heard so far.
    async fn said(&self) -> Vec<String> {
        self.0.lock().await.clone()
    }
}

/// A context whose stack answers this way, waiting the budget a real start waits.
fn ctx(health: Health) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(&LIBRARY, Lifecycle::Running, health)),
        Following::started(),
        Files::empty(),
        Source::External(project()),
        Settings {
            protocols: Protocols::both(),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .waiting(PATIENCE)
}

/// The forms an operator names.
fn named(forms: &[&str]) -> Vec<String> {
    forms.iter().map(|form| (*form).to_owned()).collect()
}

/// Start the `library` form and hand back everything the wait said.
async fn starting(health: Health) -> (Vec<String>, bool) {
    let heard = Arc::new(Heard::default());
    let ctx = ctx(health).narrating(Arc::clone(&heard) as Arc<dyn Narrator>);
    let outcome = started(&ctx, &named(&["library"]), &[], Some(0)).await;
    (heard.said().await, outcome.is_ok())
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
            "Still starting: audiobookshelf, calibre-web-automated, jellyfin, seerr \
             — 5 seconds so far, of 180."
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

/// A rehearsal waits for nothing, so it has nothing to say. It stops before the
/// single irreversible step, and the wait is on the far side of it.
#[tokio::test(start_paused = true)]
async fn a_rehearsal_waits_for_nothing_and_says_nothing() {
    let heard = Arc::new(Heard::default());
    let ctx = ctx(Health::Starting)
        .narrating(Arc::clone(&heard) as Arc<dyn Narrator>)
        .rehearsing();

    let rehearsed = dispatch(
        Command::Up {
            forms: named(&["library"]),
        },
        &ctx,
    )
    .await;

    assert!(rehearsed.is_ok(), "a rehearsal reports what would run");
    assert_eq!(heard.said().await, Vec::<String>::new());
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
