use crate::exit::{shown, success};
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_core::config::Settings;
use lemonfiber_core::ports::docker::{
    Container, Engine, ExecOutput, Failure as DockerFailure, LogLine, LogQuery, Stats, Stream,
};
use lemonfiber_core::ports::process::{Failure as RunFailure, Output, Runner};
use lemonfiber_core::stack::Source;

use super::{claimed, emit_line, kind, pull, pull_showing, released, stream, Action, Ctx};

/// A runner that answers every command the same way.
struct Answering {
    status: i32,
    stdout: &'static str,
}

#[async_trait]
impl Runner for Answering {
    async fn run(&self, _argv: &[String]) -> Result<Output, RunFailure> {
        Ok(Output {
            status: Some(self.status),
            stdout: self.stdout.to_owned(),
            stderr: String::new(),
        })
    }
}

/// An engine that answers nothing, which is all these paths ask of it.
struct Absent;

#[async_trait]
impl Engine for Absent {
    async fn list(&self, _project: &str) -> Result<Vec<Container>, DockerFailure> {
        Err(down())
    }
    async fn exec(&self, _c: &str, _a: &[String]) -> Result<ExecOutput, DockerFailure> {
        Err(down())
    }
    async fn stats(
        &self,
        _p: &str,
    ) -> Result<tokio::sync::mpsc::Receiver<(String, Stats)>, DockerFailure> {
        Err(down())
    }
    async fn logs(
        &self,
        _p: &str,
        _s: &[String],
        _q: LogQuery,
    ) -> Result<tokio::sync::mpsc::Receiver<LogLine>, DockerFailure> {
        Err(down())
    }
}

/// An engine that answers: one log line, and nothing running.
struct Talking;

#[async_trait]
impl Engine for Talking {
    async fn list(&self, _project: &str) -> Result<Vec<Container>, DockerFailure> {
        Ok(Vec::new())
    }
    async fn exec(&self, _c: &str, _a: &[String]) -> Result<ExecOutput, DockerFailure> {
        Err(down())
    }
    async fn stats(
        &self,
        _p: &str,
    ) -> Result<tokio::sync::mpsc::Receiver<(String, Stats)>, DockerFailure> {
        Err(down())
    }
    async fn logs(
        &self,
        _p: &str,
        _s: &[String],
        _q: LogQuery,
    ) -> Result<tokio::sync::mpsc::Receiver<LogLine>, DockerFailure> {
        let (sender, receiver) = tokio::sync::mpsc::channel(4);
        let _ = sender
            .send(LogLine {
                service: "sonarr".to_owned(),
                stream: Stream::Stdout,
                line: "started".to_owned(),
                at: None,
            })
            .await;
        Ok(receiver)
    }
}

fn down() -> DockerFailure {
    DockerFailure::Unreachable {
        reason: "nothing is running here".to_owned(),
    }
}

fn ctx(status: i32, stdout: &'static str) -> Ctx {
    // A scratch directory to materialise the embedded stack into, so a pull has
    // a compose file to be run against rather than failing before it starts.
    let dir =
        lemonfiber_fixtures::scratch::Scratch::named(&format!("engine-{status}{}", stdout.len()))
            .kept();
    let _ = std::fs::create_dir_all(&dir);
    let settings = Settings {
        stack_dir: Some(dir),
        ..Settings::default()
    };
    lemonfiber_testing::a_context()
        .runner(Arc::new(Answering { status, stdout }))
        .engine(Arc::new(Absent))
        .clock(Arc::new(lemonfiber_adapters::System))
        .over(Source::Embedded(&lemonfiber::carried::STACK))
        .settings(settings)
        .build()
}

/// A clean exit, as it reads.
///
/// On a paused clock, because a pull that cannot have the stack now waits for it
/// rather than being turned away, and the wait it eventually gives up on is five
/// minutes long. The runtime advances its own timers the moment nothing is ready,
/// so what is asserted is the end of the wait rather than the sitting through it.
#[tokio::test(start_paused = true)]
async fn a_pull_waits_for_the_stack_and_is_refused_when_the_wait_runs_out() {
    let mut ctx = ctx(0, "pulled");
    let dir = lemonfiber_fixtures::scratch::Scratch::named("pull-lock").kept();
    let _ = std::fs::create_dir_all(&dir);
    ctx.settings.env_file = Some(dir.join(".env"));

    let held = claimed(&ctx, Action::Up.name()).await;
    assert!(held.is_ok(), "nothing held it, so the first run took it");

    let code = pull(&ctx, &["tv".to_owned()], false).await;

    assert_ne!(
        format!("{code:?}"),
        success(),
        "a pull asked for on its own is a lifecycle operation, so a stack \
         somebody else is working on is one it waits for and then reports"
    );

    if let Ok(claim) = held {
        released(&ctx, claim).await;
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_pull_that_the_engine_refuses_is_not_a_success() {
    let forms = vec!["tv".to_owned()];
    assert_ne!(
        format!("{:?}", pull(&ctx(1, ""), &forms, false).await),
        success()
    );
    // As JSON it still reports rather than printing prose.
    assert_ne!(
        format!("{:?}", pull(&ctx(1, ""), &forms, true).await),
        success()
    );
}

#[tokio::test]
async fn a_pull_that_completes_lets_the_caller_go_on() {
    let forms = vec!["tv".to_owned()];
    assert!(pull_showing(&ctx(0, "pulled"), &forms, false).await.is_ok());
    // A rehearsal shows the command rather than fetching anything.
    let mut rehearsing = ctx(0, "");
    rehearsing.dry_run = true;
    assert!(pull_showing(&rehearsing, &forms, false).await.is_ok());
}

#[tokio::test]
async fn a_pull_that_failed_stops_what_it_was_for() {
    // Starting against images that never arrived is worse than not starting.
    let forms = vec!["tv".to_owned()];
    assert!(pull_showing(&ctx(1, ""), &forms, false).await.is_err());
}

#[test]
fn both_shapes_a_pull_line_takes_are_reachable() {
    // Both are exercised for their own sake: a person reads one, a script the
    // other, and neither should ever be handed the wrong shape.
    emit_line(kind::PULL, "Pulling sonarr", false);
    emit_line(kind::PULL, "Pulling sonarr", true);
}

/// A context whose engine answers, for the paths that need one to.
fn answering() -> Ctx {
    let mut ctx = ctx(0, "");
    ctx.seams.engine = Arc::new(Talking);
    ctx
}

#[tokio::test]
async fn streaming_reads_each_line_for_a_person_and_for_a_script() {
    // Both shapes, because a person reads one and a script the other.
    assert_eq!(
        format!(
            "{:?}",
            stream(&answering(), &[], &[], false, 10, false).await
        ),
        success()
    );
    assert_eq!(
        format!(
            "{:?}",
            stream(&answering(), &[], &[], false, 10, true).await
        ),
        success()
    );
}

/// An engine that answers with no log lines at all.
struct Quiet;

#[async_trait]
impl Engine for Quiet {
    async fn list(&self, _project: &str) -> Result<Vec<Container>, DockerFailure> {
        Ok(Vec::new())
    }
    async fn exec(&self, _c: &str, _a: &[String]) -> Result<ExecOutput, DockerFailure> {
        Err(down())
    }
    async fn stats(
        &self,
        _p: &str,
    ) -> Result<tokio::sync::mpsc::Receiver<(String, Stats)>, DockerFailure> {
        Err(down())
    }
    async fn logs(
        &self,
        _p: &str,
        _s: &[String],
        _q: LogQuery,
    ) -> Result<tokio::sync::mpsc::Receiver<LogLine>, DockerFailure> {
        let (_sender, receiver) = tokio::sync::mpsc::channel(1);
        Ok(receiver)
    }
}

#[tokio::test]
async fn a_stack_that_said_nothing_says_so_rather_than_printing_emptiness() {
    let mut ctx = ctx(0, "");
    ctx.seams.engine = Arc::new(Quiet);
    assert_eq!(
        format!("{:?}", stream(&ctx, &[], &[], false, 10, false).await),
        success()
    );
    // Asked for as JSON, silence is simply no lines rather than a note.
    assert_eq!(
        format!("{:?}", stream(&ctx, &[], &[], false, 10, true).await),
        success()
    );
    assert!(Quiet.exec("c", &[]).await.is_err());
    assert!(Quiet.stats("p").await.is_err());
}

#[tokio::test]
async fn a_pull_that_worked_reports_it() {
    let forms = vec!["tv".to_owned()];
    assert_eq!(
        format!("{:?}", pull(&ctx(0, "pulled"), &forms, false).await),
        success()
    );
}

#[tokio::test]
async fn a_rehearsed_pull_that_cannot_be_described_is_reported() {
    // No stack to describe the command against, so there is nothing to show.
    let mut rehearsing = ctx(0, "");
    rehearsing.dry_run = true;
    rehearsing.settings.stack_dir = None;
    let refused = pull_showing(&rehearsing, &["tv".to_owned()], false).await;
    assert!(refused.is_err());
}

#[tokio::test]
async fn the_capabilities_these_reads_never_ask_for_answer_that_plainly() {
    let engine = Absent;
    assert!(engine.list("p").await.is_err());
    assert!(Talking.list("p").await.is_ok());
    assert!(Quiet.list("p").await.is_ok());
    assert!(engine.exec("c", &[]).await.is_err());
    assert!(engine.stats("p").await.is_err());
    assert!(engine.logs("p", &[], LogQuery::recent(10)).await.is_err());
    assert!(Talking.exec("c", &[]).await.is_err());
    assert!(Talking.stats("p").await.is_err());
}

#[tokio::test]
async fn streaming_logs_from_a_stack_that_is_not_there_is_not_a_success() {
    let code = stream(&ctx(1, ""), &[], &[], false, 10, false).await;
    assert_ne!(shown(code), success());
}
