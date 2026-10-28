//! A start, a restart and a switch refused where a directory the project mounts into a
//! service is reached through a link.

use super::*;
use crate::app::command::Restarting;
use crate::error::codes::life::MOUNT_LINKED;

/// A machine running the stack this build carries beneath a project directory of its
/// own, its engine reporting `up` running, with `service`'s own directory a link to one
/// outside the project; answers what the runner was handed.
fn linked(name: &str, up: &[&str], service: &str) -> (Ctx, Arc<Recording>) {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let env = crate::test_support::env_without_password(name);
    let project = env.with_file_name("stack");
    let mut ctx = a_context()
        .over(crate::test_support::embedded())
        .runner(runner.clone())
        .engine(Arc::new(Reporting::holding(
            up,
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        })
        .build()
        .with_http(Fake::silent())
        .with_patience(Duration::ZERO);
    ctx.settings.stack_dir = Some(project.clone());
    ctx.settings.env_file = Some(env);
    lemonfiber_fixtures::scratch::linked_away(&project, service);
    (ctx, runner)
}

/// The forms an operator names.
fn library() -> Vec<String> {
    vec!["library".to_owned()]
}

/// The code a command was refused with, where it was.
fn refused(outcome: Result<Outcome, Box<super::super::Problem>>) -> Option<crate::error::Code> {
    outcome.err().map(|problem| problem.code)
}

#[tokio::test]
async fn a_start_over_a_linked_directory_starts_nothing() {
    let (ctx, runner) = linked("linking-up", &LIBRARY, "navidrome");
    let outcome = dispatch(Command::Up { forms: library() }, &ctx).await;
    assert_eq!(refused(outcome), Some(MOUNT_LINKED));
    assert!(!runner.ran("up"), "{:?}", runner.seen());
}

#[tokio::test]
async fn a_restart_over_a_linked_directory_restarts_nothing() {
    let (ctx, runner) = linked("linking-restart", &LIBRARY, "navidrome");
    let restart = Command::Restart(Restarting {
        forms: library(),
        services: Vec::new(),
        offer: None,
    });
    assert_eq!(refused(dispatch(restart, &ctx).await), Some(MOUNT_LINKED));
    assert!(!runner.ran("restart"), "{:?}", runner.seen());
}

#[tokio::test]
async fn a_streamed_start_over_a_linked_directory_opens_no_stream() {
    let (ctx, runner) = linked("linking-stream", &LIBRARY, "navidrome");
    let opened = crate::app::start_progress(&ctx, &library(), &[]).await;
    assert_eq!(opened.err().map(|problem| problem.code), Some(MOUNT_LINKED));
    assert!(!runner.ran("up"), "{:?}", runner.seen());
}

#[tokio::test]
async fn a_switch_over_a_linked_directory_stops_and_starts_nothing() {
    let (ctx, runner) = linked("linking-switch", &["jellyfin", "qbittorrent"], "navidrome");
    let outcome = dispatch(Command::Switch { forms: library() }, &ctx).await;
    assert_eq!(refused(outcome), Some(MOUNT_LINKED));
    assert!(
        !runner.ran("stop") && !runner.ran("up"),
        "{:?}",
        runner.seen()
    );
}

#[tokio::test]
async fn a_stop_over_a_linked_directory_still_stops() {
    let (ctx, runner) = linked("linking-down", &LIBRARY, "navidrome");
    let down = Command::Down(crate::app::Teardown {
        forms: library(),
        wait: Waiting::Never,
    });
    assert_eq!(refused(dispatch(down, &ctx).await), None);
    assert!(runner.ran("down"), "{:?}", runner.seen());
}
