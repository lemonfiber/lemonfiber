//! A start that did not complete still says where what it addressed stands.
//!
//! From here rather than from a `#[cfg(test)]` module because both starts are `async`,
//! and an async path exercised only in-crate has its coverage counted from the copy
//! that never ran.
//!
//! The case a handset found: a download client whose tunnel crash-looped was left
//! created, and the start's report named no service at all, so the one question the
//! operator had — what did not come back — was answered with nothing.

use std::sync::Arc;
use std::time::Duration;

use lemonfiber_core::app::{dispatch, started, Command, Ctx, Outcome};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::docker::State;
use lemonfiber_core::model::LifecycleReport;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::process::Output;
use lemonfiber_core::ports::Narrator;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::heard::Heard;
use lemonfiber_fixtures::ports::Following;
use lemonfiber_fixtures::support::{Reporting, Scripted};

/// Everything the `library` form declares.
const LIBRARY: [&str; 8] = [
    "jellyfin",
    "door",
    "seerr",
    "request-gate",
    "decline",
    "calibre-web-automated",
    "audiobookshelf",
    "navidrome",
];

/// A start whose Compose ended on this status, over services the engine reports this
/// way, with the budget a real start is given.
fn ctx(status: i32, lifecycle: Lifecycle, health: Health) -> Ctx {
    lemonfiber_testing::a_context()
        .runner(Arc::new(Scripted(Ok(Output {
            status: Some(status),
            stdout: String::new(),
            stderr: String::new(),
        }))))
        .engine(Arc::new(Reporting::holding(&LIBRARY, lifecycle, health)))
        .clock(Following::started())
        .filesystem(Files::empty())
        .images(lemonfiber_fixtures::pulled::Pulled::holding(Vec::new()))
        .settings(Settings {
            protocols: Protocols::both(),
            ..Settings::default()
        })
        .build()
        .with_patience(Duration::from_secs(180))
}

/// The forms an operator names.
fn named(forms: &[&str]) -> Vec<String> {
    forms.iter().map(|form| (*form).to_owned()).collect()
}

/// The ids of what a report read, and the state each was in.
fn read(report: &LifecycleReport) -> Vec<(String, State)> {
    let mut read: Vec<(String, State)> = report
        .services
        .iter()
        .map(|service| (service.id.clone(), service.state))
        .collect();
    read.sort();
    read
}

/// Every one of the library's services, in this state.
fn all(state: State) -> Vec<(String, State)> {
    let mut all: Vec<(String, State)> =
        LIBRARY.iter().map(|id| ((*id).to_owned(), state)).collect();
    all.sort();
    all
}

/// The streamed start reads every service it addressed once and does not wait on any:
/// services still starting after a failed start are reported as starting, at once, and
/// nothing is said about a wait that never happened.
#[tokio::test(start_paused = true)]
async fn a_streamed_start_that_failed_says_where_each_service_stands_without_waiting() {
    let heard = Arc::new(Heard::default());
    let ctx = ctx(1, Lifecycle::Running, Health::Starting)
        .with_narrator(Arc::clone(&heard) as Arc<dyn Narrator>);

    let report = started(&ctx, &named(&["library"]), &[], Some(1)).await;

    assert_eq!(report.as_ref().map(read).ok(), Some(all(State::Starting)));
    assert!(
        report.is_ok_and(|report| report.status == Some(1) && report.condition.is_some()),
        "the failure is still the status, and the services come with a condition"
    );
    assert_eq!(
        heard.said(),
        Vec::<String>::new(),
        "and nothing was waited on"
    );
}

/// The waited-on start does the same, and a service the start left created is
/// reported as failed rather than as something the operator turned off.
#[tokio::test(start_paused = true)]
async fn a_start_that_failed_reports_a_service_it_left_created_as_failed() {
    let ctx = ctx(1, Lifecycle::Created, Health::None);

    let outcome = dispatch(
        Command::Up {
            forms: named(&["library"]),
        },
        &ctx,
    )
    .await;

    let report = match outcome {
        Ok(Outcome::Lifecycle(report)) => Some(report),
        _ => None,
    };
    assert_eq!(report.as_ref().map(read), Some(all(State::Failed)));
    assert!(report.is_some_and(|report| report.status == Some(1)));
}

/// A start that succeeded is unchanged: it waits, and reports what settled.
#[tokio::test(start_paused = true)]
async fn a_start_that_succeeded_still_waits_for_what_settles() {
    let ctx = ctx(0, Lifecycle::Running, Health::Healthy);

    let report = started(&ctx, &named(&["library"]), &[], Some(0)).await;

    assert_eq!(report.as_ref().map(read).ok(), Some(all(State::Healthy)));
}

/// A rehearsal is unchanged too: it spawns nothing, so it reads nothing.
#[tokio::test(start_paused = true)]
async fn a_rehearsed_start_reads_no_service() {
    let ctx = ctx(1, Lifecycle::Created, Health::None).rehearsing();

    let outcome = dispatch(
        Command::Up {
            forms: named(&["library"]),
        },
        &ctx,
    )
    .await;

    assert!(matches!(
        outcome,
        Ok(Outcome::Lifecycle(report)) if report.services.is_empty() && report.status.is_none()
    ));
}

/// A form whose start left one of its services created still counts as the form the
/// operator started: that service did not come up, which is its state, and nothing
/// about it was turned off.
#[tokio::test]
async fn a_service_left_created_keeps_its_form_among_the_active_ones() {
    let (created, rest) = LIBRARY.split_at(1);
    let ctx = lemonfiber_testing::a_context()
        .engine(Arc::new(
            Reporting::holding(rest, Lifecycle::Running, Health::Healthy).alongside(
                Reporting::holding(created, Lifecycle::Created, Health::None),
            ),
        ))
        .filesystem(Files::empty())
        .settings(Settings {
            protocols: Protocols::both(),
            ..Settings::default()
        })
        .build();

    let status = dispatch(Command::Status { forms: Vec::new() }, &ctx).await;

    let active = match status {
        Ok(Outcome::Status(report)) => report.active_forms,
        _ => Vec::new(),
    };
    assert!(active.contains(&"library".to_owned()), "{active:?}");
}
