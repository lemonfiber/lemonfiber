//! Making named forms the active set, driven through the dispatcher.
//!
//! From here rather than only from a `#[cfg(test)]` module for the reason the forms
//! listing is: the app layer is compiled twice, and a path exercised only in-crate has
//! its coverage counted from the copy that never ran.
//!
//! Rehearsed except for the last one. A switch decides what to move by *reading* what
//! is running, and reading is not acting, so most of what this asserts is settled
//! before either Compose invocation would be handed to a process. The exception drives
//! both of them through a scripted runner, because the path a real switch takes after
//! the rehearsal ends is exactly the part a rehearsal cannot reach.

mod common;

use common::stack::project;
use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::model::Switched;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};

/// A context whose engine reports the named services up, and nothing else.
fn ctx(up: &[&str]) -> Ctx {
    Ctx::new(
        Arc::new(lemonfiber_fixtures::ports::Idle),
        Arc::new(Reporting::holding(up, Lifecycle::Running, Health::Healthy)),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_fixtures::files::Files::empty(),
        Source::External(project()),
        Settings {
            protocols: Protocols::both(),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .rehearsing()
}

/// What the switch reported moving.
async fn switching_to(form: &str, up: &[&str]) -> Option<Switched> {
    match dispatch(
        Command::Switch {
            forms: vec![form.to_owned()],
        },
        &ctx(up),
    )
    .await
    {
        Ok(Outcome::Lifecycle(report)) => report.switched,
        _ => None,
    }
}

/// The requirement in one sentence: what both shapes hold is left alone, and only
/// what fell outside the new one is stopped.
#[tokio::test]
async fn narrowing_stops_only_what_left_the_closure() {
    let switched = switching_to("library", &["jellyfin", "qbittorrent"]).await;

    assert_eq!(
        switched.as_ref().map(|moved| moved.stopped.clone()),
        Some(vec!["qbittorrent".to_owned()]),
        "the torrent client is outside `library`, and it is the only thing stopped: \
         {switched:?}"
    );
    assert!(
        switched
            .as_ref()
            .is_some_and(|moved| moved.kept.contains(&"jellyfin".to_owned())),
        "and the media server, which both shapes hold, is not restarted to get there: \
         {switched:?}"
    );
}

/// Compose refuses a service name whose profile is not active, so a stop that named
/// the profiles it was arriving at would run and change nothing.
#[tokio::test]
async fn the_stop_carries_the_profiles_it_is_leaving() {
    let command = switching_to("library", &["qbittorrent"])
        .await
        .and_then(|moved| moved.stop_command);

    assert!(
        command.as_ref().is_some_and(|argv| argv
            .windows(2)
            .any(|pair| pair == ["--profile".to_owned(), "torrent".to_owned()])),
        "{command:?}"
    );
    assert!(
        command.as_ref().is_some_and(|argv| argv
            .windows(2)
            .any(|pair| pair == ["--".to_owned(), "qbittorrent".to_owned()])),
        "and the service is fenced off from option parsing: {command:?}"
    );
}

#[tokio::test]
async fn a_switch_onto_a_stack_that_is_already_in_that_shape_moves_nothing() {
    let switched = switching_to("library", &["jellyfin"]).await;

    assert!(
        switched
            .as_ref()
            .is_some_and(|moved| moved.stopped.is_empty() && moved.stop_command.is_none()),
        "nothing fell outside, so there is no stop to run: {switched:?}"
    );
    assert!(
        switched
            .as_ref()
            .is_some_and(|moved| moved.kept.contains(&"jellyfin".to_owned())),
        "{switched:?}"
    );
}

/// A rehearsal is worth having here precisely because it says what would stop, so it
/// has to report the same thing a real run would act on.
#[tokio::test]
async fn a_rehearsed_switch_says_what_it_would_stop_without_stopping_it() {
    let outcome = dispatch(
        Command::Switch {
            forms: vec!["library".to_owned()],
        },
        &ctx(&["qbittorrent"]),
    )
    .await;

    assert!(
        matches!(&outcome, Ok(Outcome::Lifecycle(report))
            if report.rehearsed
                && report.status.is_none()
                && report.action == "switch"
                && report
                    .switched
                    .as_ref()
                    .is_some_and(|moved| !moved.stopped.is_empty())),
        "a rehearsal reports the move and runs neither command"
    );
}

/// Everything after the rehearsal: the stop is run, the start is run after it, and the
/// switch waits for what it started. Driven from here as well as from the crate's own
/// tests because this file is compiled against the library rather than into it, and a
/// path exercised only on the other side of that line is a path this side never ran.
#[tokio::test]
async fn a_real_switch_stops_then_starts_then_waits() {
    let ctx = Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(
            &[
                "jellyfin",
                "seerr",
                "calibre-web-automated",
                "audiobookshelf",
                "qbittorrent",
            ],
            Lifecycle::Running,
            Health::Healthy,
        )),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_fixtures::files::Files::empty(),
        Source::External(project()),
        Settings {
            protocols: Protocols::both(),
            ..Settings::default()
        },
        Environment::MacOs,
    );

    let outcome = dispatch(
        Command::Switch {
            forms: vec!["library".to_owned()],
        },
        &ctx,
    )
    .await;

    assert!(
        matches!(&outcome, Ok(Outcome::Lifecycle(report))
            if !report.rehearsed
                && report.status == Some(0)
                && report.condition.is_some()
                && report
                    .switched
                    .as_ref()
                    .is_some_and(|moved| moved.stopped == ["qbittorrent".to_owned()])),
        "the torrent client is stopped, the library form is started, and the run waits \
         for it to be usable before saying what it came to"
    );
}
