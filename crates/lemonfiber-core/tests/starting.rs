//! A start narrated as it happens, driven through the core rather than a surface.
//!
//! From here rather than from a `#[cfg(test)]` module because both halves are `async`,
//! and an async path exercised only in-crate has its coverage counted from the copy
//! that never ran.
//!
//! The two halves are tested apart because they are apart: the stream says what
//! happened as it happened, and the report says what it amounts to. What they share
//! is the plan they are both about, and that is asserted by neither — it is the same
//! `readied` the waited-on path uses, which is the point of it being shared.

mod common;

use common::stack::project;
use std::sync::Arc;
use std::time::Duration;

use lemonfiber_core::app::{start_progress, started, Ctx, Outcome};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::process::{Failure, Output, Progress};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};

/// The services this stack's `library` form holds.
const LIBRARY: [&str; 4] = ["jellyfin", "sonarr", "radarr", "prowlarr"];

/// A context whose Compose answers this way, with the stack reported healthy.
fn ctx(compose: Result<Output, Failure>) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(compose)),
        Arc::new(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            Health::Healthy,
        )),
        lemonfiber_fixtures::ports::Stopped::at(1_786_968_000),
        lemonfiber_fixtures::files::Files::empty(),
        Source::External(project()),
        Settings {
            protocols: Protocols::both(),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .waiting(Duration::ZERO)
}

/// The forms an operator names.
fn named(forms: &[&str]) -> Vec<String> {
    forms.iter().map(|form| (*form).to_owned()).collect()
}

/// Everything the start said and the status it ended on, or nothing where it could
/// not be spawned at all.
async fn narrated(ctx: &Ctx) -> Option<(Vec<String>, Option<i32>)> {
    let mut progress = start_progress(ctx, &named(&["library"])).await.ok()?;
    let mut said = Vec::new();
    let mut status = None;
    while let Some(event) = progress.recv().await {
        match event {
            Progress::Line(line) => said.push(line),
            Progress::Ended(code) => status = code,
        }
    }
    Some((said, status))
}

/// The whole point: what Compose says reaches the caller while it is being said,
/// rather than being swallowed and summarised once it is over.
#[tokio::test]
async fn a_start_hands_back_what_compose_says_as_it_says_it() {
    let ctx = ctx(Ok(spoke(
        "Container lf-sonarr  Started\nContainer lf-radarr  Started",
    )));

    assert_eq!(
        narrated(&ctx).await,
        Some((
            vec![
                "Container lf-sonarr  Started".to_owned(),
                "Container lf-radarr  Started".to_owned(),
            ],
            Some(0)
        )),
        "every line, in order, and the status it ended on"
    );
}

/// A Compose that is not installed is not a start that produced no output — the
/// difference is the whole of what a surface has to tell the operator.
#[tokio::test]
async fn a_start_that_cannot_be_spawned_is_refused_rather_than_silent() {
    let missing = Failure::NotFound {
        program: "docker".to_owned(),
    };

    let opened = start_progress(&ctx(Err(missing)), &named(&["library"])).await;

    assert!(opened.is_err(), "the spawn failure reaches the caller");
}

/// A start that worked waits for its services, because "started" that means "a
/// process exists" is a claim the operator will disprove by opening a browser.
#[tokio::test]
async fn a_start_that_succeeded_waits_for_its_services() {
    let ctx = ctx(Ok(spoke("")));

    let report = match started(&ctx, &named(&["library"]), Some(0)).await {
        Ok(Outcome::Lifecycle(report)) => Some(report),
        _ => None,
    };

    assert_eq!(
        report.as_ref().map(|report| report.status),
        Some(Some(0)),
        "the status it was given is the status it reports"
    );
    assert!(
        report.is_some_and(|report| !report.services.is_empty()),
        "and it waited to find out what each service ended up doing"
    );
}

/// Nothing is waited on after a Compose invocation that failed: there is nothing to
/// wait for, and waiting would turn a fast failure into a slow one.
#[tokio::test]
async fn a_start_that_failed_is_not_then_waited_on() {
    let ctx = ctx(Ok(spoke("")));

    let report = match started(&ctx, &named(&["library"]), Some(1)).await {
        Ok(Outcome::Lifecycle(report)) => Some(report),
        _ => None,
    };

    assert_eq!(
        report.map(|report| (report.status, report.services.is_empty())),
        Some((Some(1), true))
    );
}
