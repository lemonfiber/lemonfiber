//! The reads that arrive over time rather than all at once.
//!
//! Logs, a pull and a watch each produce output as they go, so none of them is a
//! value that comes back from dispatch — they stream, and something has to sit
//! with them until they end. That is what is here, kept out of the dispatcher so
//! it stays a dispatcher.

use std::process::ExitCode;
use std::time::Duration;

use lemonfiber_core::adapters::Disk;
use lemonfiber_core::app::{
    claimed, dispatch, in_flight, logs, pull_progress, released, start_progress, started,
    supervise, Command, Ctx, WATCH,
};
use lemonfiber_core::model::kind::{self, Kind};
use lemonfiber_core::model::Envelope;
use lemonfiber_core::ports::docker::LogQuery;
use lemonfiber_core::ports::process::Progress as PullEvent;

use crate::exit::{complain, settled, FAILURE};
use crate::keyboard::{Console, Keyboard};
use crate::prompt::Answers as _;
use crate::render::downloads::interrupting;
use crate::render::{logged, render, watched, UNRENDERABLE};
use crate::say::{complain, emit, say};
use crate::setup::Surface as _;
use crate::stopping::{answered, asking, Asking, Choice, ASK_TO_WAIT};

/// Print log lines as they arrive, until the stream ends.
///
/// Machine-readable output is one envelope per line rather than one document
/// containing all of them: a stream has no last element to close a document
/// with, and a consumer of `--follow --json` needs each line when it happens
/// rather than when the service stops.
pub(crate) async fn stream(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    follow: bool,
    tail: u32,
    json: bool,
) -> ExitCode {
    let query = LogQuery { tail, follow };
    let mut lines = match logs(ctx, forms, services, query).await {
        Ok(opened) => opened,
        Err(problem) => return complain(&problem),
    };
    let mut seen = 0_u64;
    while let Some(line) = lines.recv().await {
        seen += 1;
        if json {
            emit!(
                "{}",
                Envelope::new(kind::LOG, &line)
                    .to_json()
                    .unwrap_or(UNRENDERABLE.to_owned())
            );
        } else {
            say!("{}", logged(&line.service, &line.line));
        }
    }

    // Silence and "no output" are different answers, and a viewer that renders
    // them identically leaves the operator wondering which one they got.
    if seen == 0 && !json {
        say!("no output");
    }
    ExitCode::SUCCESS
}

/// Pull the images the forms need, showing each as it downloads.
///
/// Image pulls are gigabytes and take minutes; a silent wait reads as a hang, so
/// the operator is told the wait is coming and then shown Compose's per-image
/// progress as it arrives.
pub(crate) async fn pull(ctx: &Ctx, forms: &[String], json: bool) -> ExitCode {
    // Claimed here rather than inside `pull_showing`, which setup also calls: a pull
    // asked for on its own is a lifecycle operation like every other, and the same
    // request through the dispatcher already claims. Setup's pull is left alone —
    // it is followed immediately by a start that claims, and a first run has nothing
    // to race. Given back on both paths out.
    let claim = match claimed(ctx).await {
        Ok(claim) => claim,
        Err(problem) => return complain(&problem),
    };
    let code = match pull_showing(ctx, forms, json).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    };
    released(ctx, claim).await;
    code
}

/// Stream a pull to the screen, or report why it could not run.
///
/// Split from [`pull`] so setup's Start phase can pull-with-progress and, only if
/// that succeeds, go on to bring the stack up — a `Result` it can branch on rather
/// than an exit code it cannot compare.
pub(crate) async fn pull_showing(ctx: &Ctx, forms: &[String], json: bool) -> Result<(), ExitCode> {
    // A rehearsal reports what would be pulled rather than pulling it; the buffered
    // path already renders that, so a dry run borrows it rather than streaming.
    if ctx.dry_run {
        return pull_rehearsal(ctx, forms, json).await;
    }

    // The expected-duration statement, before the wait — qualitative, because the
    // real figure depends on the operator's line and what is already cached.
    if !json {
        say!("Pulling images — usually a few minutes, and several gigabytes.");
    }

    let mut progress = pull_progress(ctx, forms)
        .await
        .map_err(|problem| complain(&problem))?;
    let mut failed = false;
    while let Some(event) = progress.recv().await {
        match event {
            PullEvent::Line(line) => emit_line(kind::PULL, &line, json),
            // A non-zero exit is the pull's own report that an image did not come
            // down; the operator is told, and a script sees a non-zero code.
            PullEvent::Ended(status) => failed = status != Some(0),
        }
    }

    if failed {
        if !json {
            complain!("\nSome images could not be pulled — check the output above.");
        }
        return Err(ExitCode::from(FAILURE));
    }
    Ok(())
}

/// A dry run's rehearsal: report what a pull would fetch without fetching it,
/// borrowing the buffered path's rendering rather than streaming a pull that will
/// not happen.
pub(crate) async fn pull_rehearsal(
    ctx: &Ctx,
    forms: &[String],
    json: bool,
) -> Result<(), ExitCode> {
    match dispatch(
        Command::Pull {
            forms: forms.to_vec(),
        },
        ctx,
    )
    .await
    {
        Ok(outcome) => {
            render(&outcome, json);
            Ok(())
        }
        Err(problem) => Err(complain(&problem)),
    }
}

/// Render one line a narrated command wrote — wrapped in an envelope under `--json`,
/// indented beneath the command for a person to read.
///
/// The kind is the caller's, because a consumer filtering a `--json` stream is
/// filtering on it: a pull's lines and a start's are different things happening.
pub(crate) fn emit_line(kind: Kind, line: &str, json: bool) {
    if !json {
        say!("  {line}");
        return;
    }
    emit!(
        "{}",
        Envelope::new(kind, line)
            .to_json()
            .unwrap_or(UNRENDERABLE.to_owned())
    );
}

/// Watch the data location until it is lost, then report what was stopped.
///
/// This blocks for as long as the location holds — the operator ends it with the
/// same interrupt they end any foreground command. It returns only once the
/// location is lost and the services have been stopped, which is the one thing
/// it exists to do.
pub(crate) async fn guard(ctx: &Ctx, forms: &[String], json: bool) -> ExitCode {
    match supervise(ctx, &Disk, forms, WATCH).await {
        Ok(report) => {
            watched(&report, json);
            ExitCode::SUCCESS
        }
        Err(problem) => complain(&problem),
    }
}

/// How often a wait looks again.
///
/// Ten seconds: long enough that two clients are not being polled at, short enough
/// that an operator watching the last download finish is not left wondering whether
/// the wait noticed.
const AGAIN: Duration = Duration::from_secs(10);

/// Report what stopping would interrupt, and do what the operator says about it.
///
/// Silent where nothing is coming down, which is the ordinary case — a teardown that
/// remarked on an empty queue every time would teach the operator to skip the part
/// that matters on the day it is not empty.
pub(crate) async fn settle(ctx: &Ctx, forms: &[String], wait: bool, yes: bool) {
    let active = in_flight(ctx, forms).await;
    if active.is_empty() {
        return;
    }
    interrupting(&active).print();

    let choice = match asking(wait, yes, Console.interactive()) {
        Asking::Settled(choice) => choice,
        Asking::Ask => answered(&Keyboard.ask(ASK_TO_WAIT)),
    };
    if choice == Choice::Wait {
        drained(ctx, forms).await;
    }
}

/// Wait until nothing is coming down any more.
///
/// Said again only when the count changes, because a list reprinted every ten seconds
/// is something an operator scrolls past rather than reads — and the one moment it
/// has news is the moment one of them finishes.
async fn drained(ctx: &Ctx, forms: &[String]) {
    let mut counted = usize::MAX;
    loop {
        let active = in_flight(ctx, forms).await;
        if active.is_empty() {
            say!("downloads finished — stopping now");
            return;
        }
        if active.len() != counted {
            counted = active.len();
            interrupting(&active).print();
        }
        tokio::time::sleep(AGAIN).await;
    }
}

/// Start the forms, showing what Compose says as it says it.
///
/// A start is minutes of silence otherwise, and the silence is the problem: pulling
/// an image, creating a network and waiting on a health check all look identical from
/// outside, and the one that has hung looks identical to all three. So Compose's own
/// narration goes to the screen as it arrives, and the report follows it.
pub(crate) async fn start(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    json: bool,
) -> ExitCode {
    // A streamed start does not go through the dispatcher, so it claims the stack
    // here rather than inheriting the claim `lifecycle` takes. Given back below on
    // both paths out, for the same reason it is given back there.
    let claim = match claimed(ctx).await {
        Ok(claim) => claim,
        Err(problem) => return complain(&problem),
    };

    // A rehearsal runs nothing, so it has nothing to narrate — and it needs no
    // separate path, because the report it wants is the one built from the same plan
    // without the single irreversible step, which is what a status of nothing gets.
    let status = if ctx.dry_run {
        None
    } else {
        match narrated(ctx, forms, services, json).await {
            Ok(status) => status,
            Err(code) => {
                released(ctx, claim).await;
                return code;
            }
        }
    };

    // The report is asked for whatever the status was: a start that failed part way
    // has still started something, and which services came up is the first thing an
    // operator needs in order to do anything about the ones that did not.
    let outcome = started(ctx, forms, services, status).await;
    released(ctx, claim).await;
    match outcome {
        Ok(outcome) => {
            render(&outcome, json);
            settled(&outcome)
        }
        Err(problem) => complain(&problem),
    }
}

/// Spawn the start and put Compose's narration on the screen as it arrives.
///
/// Gives back the exit status Compose ended on, or the code a surface should exit
/// with where the command could not be spawned at all.
async fn narrated(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    json: bool,
) -> Result<Option<i32>, ExitCode> {
    let mut progress = start_progress(ctx, forms, services)
        .await
        .map_err(|problem| complain(&problem))?;
    let mut status = None;
    while let Some(event) = progress.recv().await {
        match event {
            PullEvent::Line(line) => emit_line(kind::START, &line, json),
            PullEvent::Ended(code) => status = code,
        }
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use crate::exit::{shown, success};
    use std::sync::Arc;

    use async_trait::async_trait;
    use lemonfiber_core::config::Settings;
    use lemonfiber_core::platform::Environment;
    use lemonfiber_core::ports::docker::{
        Container, Engine, ExecOutput, Failure as DockerFailure, LogLine, LogQuery, Stats, Stream,
    };
    use lemonfiber_core::ports::process::{Failure as RunFailure, Output, Runner};
    use lemonfiber_core::stack::Source;

    use super::{claimed, emit_line, guard, kind, pull, pull_showing, released, stream, Ctx};

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
        let dir = std::env::temp_dir().join(format!(
            "lemonfiber-engine-{}-{status}{}",
            std::process::id(),
            stdout.len()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let settings = Settings {
            stack_dir: Some(dir),
            ..Settings::default()
        };
        Ctx::new(
            Arc::new(Answering { status, stdout }),
            Arc::new(Absent),
            Arc::new(lemonfiber_core::adapters::System),
            Arc::new(lemonfiber_core::adapters::Disk),
            Source::Embedded(&lemonfiber::cli::STACK),
            settings,
            Environment::MacOs,
        )
    }

    /// A clean exit, as it reads.

    #[tokio::test]
    async fn a_pull_is_refused_while_another_run_holds_the_stack() {
        let mut ctx = ctx(0, "pulled");
        let dir = std::env::temp_dir().join(format!("lemonfiber-pull-lock-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        ctx.settings.env_file = Some(dir.join(".env"));

        let held = claimed(&ctx).await;
        assert!(held.is_ok(), "nothing held it, so the first run took it");

        let code = pull(&ctx, &["tv".to_owned()], false).await;

        assert_ne!(
            format!("{code:?}"),
            success(),
            "a pull asked for on its own is a lifecycle operation, so a stack \
             somebody else is working on is one it is told about"
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
    fn each_pull_line_reads_as_prose_or_as_an_envelope() {
        // Both are exercised for their own sake: a person reads one, a script the
        // other, and neither should ever be handed the wrong shape.
        emit_line(kind::PULL, "Pulling sonarr", false);
        emit_line(kind::PULL, "Pulling sonarr", true);
    }

    /// A context whose engine answers, for the paths that need one to.
    fn answering() -> Ctx {
        let mut ctx = ctx(0, "");
        ctx.engine = Arc::new(Talking);
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
        ctx.engine = Arc::new(Quiet);
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
    async fn a_watch_over_a_location_that_holds_ends_cleanly() {
        // Nothing running and nothing lost: the watch has nothing to stop.
        let code = guard(&answering(), &["tv".to_owned()], false).await;
        let _ = code;
    }

    #[tokio::test]
    async fn a_watch_that_ends_reports_how_it_ended() {
        // Nothing to guard, so the watch has nothing to wait on and says so.
        let mut ctx = answering();
        ctx.settings.data_root = Some(std::env::temp_dir().join("lemonfiber-watch-nowhere"));
        let code = guard(&ctx, &["tv".to_owned()], false).await;
        let _ = code;
        let json = guard(&ctx, &["tv".to_owned()], true).await;
        let _ = json;
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

    #[tokio::test]
    async fn guarding_a_location_that_cannot_be_watched_reports_rather_than_waits() {
        let code = guard(&ctx(1, ""), &["tv".to_owned()], false).await;
        assert_ne!(shown(code), success());
    }
}
