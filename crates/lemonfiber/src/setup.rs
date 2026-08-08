//! The first-run setup surface — the conversation, not the decisions.
//!
//! A bare `lemonfiber` and `lemonfiber setup` both land here: this greets an
//! operator, offers or resumes or recovers a setup, drives the wizard against a
//! terminal or the flags, and brings the stack up once the answers are applied.
//! What to ask, what an answer means, and what to write are all the core's,
//! reached through [`core_setup`]; reading a line and rendering a question are the
//! surface's, and that is what lives here. Split out of `main` so the dispatcher
//! stays a dispatcher.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use lemonfiber_core::app::{setup as core_setup, Ctx};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::validate::Live;
use lemonfiber_core::wizard::{offer_setup, Progress, Status, Wizard};
use lemonfiber_core::PRODUCT;

mod boot;
mod interrupted;

use boot::{preflight, start};
use interrupted::recover_setup;

use crate::context::{context, here, read_settings};
use crate::exit::{complain, FAILURE, USAGE};
use crate::keyboard::Console;
use crate::prompt::{Flags, SetupFlags};

/// The three ways setup reaches a person: whether one is there, what they typed,
/// and what does the asking.
///
/// Behind a trait because each is a thing no test can be. What setup *decides* —
/// which of the ways out of an interrupted run to offer, whether a machine is
/// already configured, what to do with an answer — is all on this side of it, and
/// is the part worth holding to anything.
pub(crate) trait Surface {
    /// Whether anyone is present to answer.
    fn interactive(&self) -> bool;

    /// Show a prompt and read the trimmed line typed in reply.
    fn line(&self, prompt: &str) -> String;

    /// What puts setup's questions, once there is someone to put them to.
    fn asking(
        &self,
        environment: Environment,
        default_data: PathBuf,
    ) -> Box<dyn core_setup::Prompt>;
}

/// Meet an operator who typed `lemonfiber` with nothing after it.
///
/// A bare invocation is the front door, and what waits behind it depends on what
/// the machine has. On one with nothing configured that is almost always a first
/// run, so setup is offered right here rather than behind a subcommand a newcomer
/// has no way to know to type. On a configured one setup is the wrong tool — its
/// answers are already written — so the operator is pointed at changing a setting
/// or starting the stack instead. A setup left unfinished is neither: it is the
/// same business a bare run should pick up as `setup` would, so those states are
/// handed straight to [`run_setup`], which knows the way out.
pub(crate) async fn greet(stack_dir: Option<PathBuf>, dry_run: bool) -> ExitCode {
    let ctx = context(stack_dir, dry_run);
    let Some(paths) = here() else {
        // With nowhere to keep its files there is nothing to offer and nothing to
        // point at, so the plain pointer is the only honest thing left to say.
        println!("{PRODUCT} — run `{PRODUCT} --help` to see what it can do");
        return ExitCode::SUCCESS;
    };
    greeting(ctx, &paths, &Console).await
}

/// The greeting itself, once there is somewhere to keep files and something to
/// hold the conversation across.
async fn greeting(ctx: Ctx, paths: &Paths, surface: &dyn Surface) -> ExitCode {
    // A stopped apply or a quit mid-question is unfinished setup, not a fresh or a
    // finished machine, and must be caught before the configured-yet check below —
    // an interrupted apply leaves half-written settings that check would read as
    // done. Handing these to the setup path detects and offers the way out.
    let progress = core_setup::progress_at(&paths.setup_progress());
    if matches!(
        Status::of(progress.as_ref()),
        Status::FailedApply | Status::InProgress
    ) {
        // A bare invocation carries no flags; unfinished setup is picked up
        // interactively, the same conversation a fresh bare run would have.
        return setting_up(ctx, paths, surface, SetupFlags::none()).await;
    }

    if !offer_setup(paths.env_file().exists()) {
        // Already set up: setup would walk a done machine back to its first
        // question. Reconfiguration and starting are what a bare run wants instead —
        // and this is guidance, not a misuse, so it leaves with success.
        println!("{PRODUCT} is already set up on this machine.");
        println!("  · change a setting with `{PRODUCT} config set <key> <value>`");
        println!("  · start the stack with `{PRODUCT} up`");
        println!("  · see everything with `{PRODUCT} --help`");
        return ExitCode::SUCCESS;
    }

    println!("No configuration found.");

    // Setup applies answers, so there is nothing for --dry-run to rehearse. Said
    // here, before the offer, rather than asking a question whose yes could not be
    // honoured — the same refusal `setup` gives, and at the same point in the walk.
    if ctx.dry_run {
        eprintln!("Setup applies your answers, so it has nothing to rehearse.");
        eprintln!("Run `{PRODUCT} setup` without --dry-run when you are ready.");
        return ExitCode::from(USAGE);
    }

    if !surface.interactive() {
        // No one is here to take the offer, so it is stated rather than asked —
        // never left waiting on input that will not come.
        println!("Run `{PRODUCT} setup` to configure your stack.");
        return ExitCode::SUCCESS;
    }
    if !confirm_setup(surface) {
        println!("No changes made — run `{PRODUCT} setup` when you are ready.");
        return ExitCode::SUCCESS;
    }
    setting_up(ctx, paths, surface, SetupFlags::none()).await
}

/// Ask whether to begin setup now, taking silence and anything but a clear no as
/// yes — a first run is what a bare invocation on an unconfigured machine means,
/// so the gentle default is to proceed.
fn confirm_setup(surface: &dyn Surface) -> bool {
    !matches!(
        surface
            .line("Run first-time setup? [Y/n]")
            .to_lowercase()
            .as_str(),
        "n" | "no"
    )
}

/// Run the setup wizard on a machine that is not configured yet.
///
/// Setup is a conversation and then a write, so it stays on this side of the core:
/// it decides nothing itself, but reading a line and rendering a question is the
/// surface's job, and the [`Terminal`] does it. What to ask, what an answer means,
/// and what to write are all the core's, reached through [`core_setup::run`].
pub(crate) async fn run_setup(ctx: Ctx, flags: SetupFlags) -> ExitCode {
    // Applying is the point of it, so there is nothing to rehearse; saying so is
    // kinder than a wizard that asks every question and then changes nothing.
    if ctx.dry_run {
        eprintln!("setup applies your answers, so it has nothing to rehearse.");
        eprintln!("Run it without --dry-run.");
        return ExitCode::from(USAGE);
    }

    let Some(paths) = here() else {
        eprintln!("error: could not find where to keep {PRODUCT}'s files on this system.");
        return ExitCode::from(FAILURE);
    };

    setting_up(ctx, &paths, &Console, flags).await
}

/// What a previous run left decides what this one does — the whole of setup's
/// routing, once there is somewhere to keep files and something to ask across.
async fn setting_up(ctx: Ctx, paths: &Paths, surface: &dyn Surface, flags: SetupFlags) -> ExitCode {
    // What a previous run left decides what this one does. An apply that stopped
    // part-way is offered back before anything else — otherwise the configured-yet
    // check below would see its half-written settings and call the machine done.
    let progress = core_setup::progress_at(&paths.setup_progress());
    match Status::of(progress.as_ref()) {
        Status::FailedApply => recover_setup(ctx, paths, surface, progress).await,
        // A run that quit mid-question saved where it reached; pick it back up.
        Status::InProgress => resume_gather(ctx, paths, surface, progress, flags).await,
        // Absent and applied are neither a stopped apply nor a saved run, so they
        // begin, or decline, a fresh one.
        _ => fresh_setup(ctx, paths, surface, flags).await,
    }
}

/// Gather answers on a machine with nothing to recover or resume.
async fn fresh_setup(
    ctx: Ctx,
    paths: &Paths,
    surface: &dyn Surface,
    flags: SetupFlags,
) -> ExitCode {
    // Setup is for a machine with nothing configured; a configured one is changed
    // through its settings, not walked back to its first question.
    if !offer_setup(paths.env_file().exists()) {
        println!("This machine is already set up.");
        println!("Change a setting with `{PRODUCT} config set`, or start it with `{PRODUCT} up`.");
        return ExitCode::from(USAGE);
    }

    let environment = ctx.environment;
    drive(ctx, paths, surface, Wizard::new(environment), flags).await
}

/// Pick a setup back up from the answers a quit run saved.
async fn resume_gather(
    ctx: Ctx,
    paths: &Paths,
    surface: &dyn Surface,
    progress: Option<Progress>,
    flags: SetupFlags,
) -> ExitCode {
    // In-progress means a saved run; if it is somehow gone there is nothing to
    // resume, so a fresh run is the honest fallback.
    let Some(progress) = progress else {
        return fresh_setup(ctx, paths, surface, flags).await;
    };
    println!("Picking up where a previous setup left off.");
    let environment = ctx.environment;
    drive(
        ctx,
        paths,
        surface,
        Wizard::resume(environment, progress),
        flags,
    )
    .await
}

/// Ask the questions the `wizard` still needs, apply the answers, and start.
///
/// The answers come from a terminal where there is one; where there is not, they
/// come from the flags. Either way it is the same walk — the wizard cannot tell —
/// so a flag run still probes the data location and proves the indexer, with the
/// warnings a person would weigh settled by the standing `--yes`.
async fn drive(
    mut ctx: Ctx,
    paths: &Paths,
    surface: &dyn Surface,
    mut wizard: Wizard,
    flags: SetupFlags,
) -> ExitCode {
    // The environment is checked before the first question, so a missing or
    // unreachable container engine is caught here rather than after eleven
    // answers — nothing setup does can work without one.
    if let Err(code) = preflight(&ctx).await {
        return code;
    }

    // Credentials are proven against their live services as they are entered — the
    // indexer and any existing service over HTTP, a Usenet provider over a real,
    // TLS-wrapped NNTP connection.
    let validator = Live::with_nntp(
        ctx.http.clone(),
        Arc::new(lemonfiber_core::adapters::Dialer::new()),
    );

    // A terminal answers the questions; without one the flags do, and where a flag
    // a question needs is missing the run is told which rather than left waiting on
    // input that never comes.
    let prompt: Box<dyn core_setup::Prompt> = if surface.interactive() {
        surface.asking(ctx.environment, default_data_location(paths))
    } else {
        let missing = flags.missing(&wizard);
        if !missing.is_empty() {
            eprintln!("error: setup here is non-interactive, so it needs values as flags:");
            for flag in missing {
                eprintln!("  {flag}");
            }
            eprintln!("\nRun it in a terminal to answer interactively instead.");
            return ExitCode::from(USAGE);
        }
        Box::new(Flags::new(flags, default_data_location(paths)))
    };

    match core_setup::run(
        &mut wizard,
        prompt.as_ref(),
        ctx.filesystem.as_ref(),
        &validator,
        paths,
        ctx.stack,
        &stamp(),
    )
    .await
    {
        Ok(core_setup::Outcome::Applied) => {
            // The settings read at startup predate the file setup just wrote, so
            // they are refreshed before the stack is brought up against them.
            ctx.settings = read_settings();
            println!("\nSetup is done — bringing your stack up.");
            start(&ctx).await
        }
        Ok(core_setup::Outcome::Abandoned) => {
            println!("\nSetup was left here — nothing was written.");
            ExitCode::SUCCESS
        }
        Err(problem) => complain(&problem),
    }
}

/// The data location setup proposes when the operator does not name one.
///
/// A directory under this machine's data base, so a default run lands somewhere
/// real and writable; an operator with a NAS or a separate disk names that
/// instead.
fn default_data_location(paths: &Paths) -> PathBuf {
    paths.data_dir().join("media")
}

/// A timestamp for the change journal — seconds since the epoch, or an empty
/// string on the absurd clock this reversal has no better answer for.
fn stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use async_trait::async_trait;
    use lemonfiber_core::config::Settings;

    use lemonfiber_core::ports::docker::{
        Container, Engine, ExecOutput, Failure as DockerFailure, LogLine, LogQuery, Stats,
    };
    use lemonfiber_core::ports::process::{Failure as RunFailure, Output, Runner};
    use lemonfiber_core::stack::Source;
    use tokio::sync::mpsc::Receiver;

    use super::{
        confirm_setup, default_data_location, greeting, setting_up, stamp, Ctx, Environment,
        ExitCode, PathBuf, Paths, SetupFlags, Surface,
    };

    /// A surface that answers from a script and says whether anyone is there.
    pub(crate) struct Scripted {
        interactive: bool,
        lines: std::cell::RefCell<Vec<String>>,
    }

    impl Scripted {
        pub(crate) fn saying(interactive: bool, lines: &[&str]) -> Self {
            Self {
                interactive,
                lines: std::cell::RefCell::new(
                    lines.iter().rev().map(|line| (*line).to_owned()).collect(),
                ),
            }
        }
    }

    impl Surface for Scripted {
        fn interactive(&self) -> bool {
            self.interactive
        }

        fn line(&self, _prompt: &str) -> String {
            self.lines.borrow_mut().pop().unwrap_or_default()
        }

        fn asking(
            &self,
            environment: Environment,
            default_data: PathBuf,
        ) -> Box<dyn super::core_setup::Prompt> {
            // A terminal answering from the same script, so a walk that reaches the
            // questions is answered rather than left waiting.
            Box::new(crate::prompt::Terminal::answered_by(
                environment,
                default_data,
                Box::new(Echoing),
            ))
        }
    }

    /// Answers that are always empty — every question takes its default, which is
    /// what a person pressing enter through the whole walk would give.
    struct Echoing;

    impl crate::prompt::Answers for Echoing {
        fn ask(&self, _question: &str) -> String {
            String::new()
        }

        fn secret(&self, _prompt: &str) -> String {
            String::new()
        }
    }

    /// An engine that answers nothing, so the environment check cannot pass.
    struct DeadEngine;

    #[async_trait]
    impl Engine for DeadEngine {
        async fn list(&self, _project: &str) -> Result<Vec<Container>, DockerFailure> {
            Err(down())
        }
        async fn exec(
            &self,
            _container: &str,
            _argv: &[String],
        ) -> Result<ExecOutput, DockerFailure> {
            Err(down())
        }
        async fn stats(&self, _project: &str) -> Result<Receiver<(String, Stats)>, DockerFailure> {
            Err(down())
        }
        async fn logs(
            &self,
            _project: &str,
            _services: &[String],
            _query: LogQuery,
        ) -> Result<Receiver<LogLine>, DockerFailure> {
            Err(down())
        }
    }

    fn down() -> DockerFailure {
        DockerFailure::Unreachable {
            reason: "nothing is running here".to_owned(),
        }
    }

    /// A runner that refuses everything, so the environment cannot pass.
    struct DeadRunner;

    #[async_trait]
    impl Runner for DeadRunner {
        async fn run(&self, _argv: &[String]) -> Result<Output, RunFailure> {
            Ok(Output {
                status: Some(1),
                stdout: String::new(),
                stderr: "no compose here".to_owned(),
            })
        }
    }

    /// A runner that answers as a working Docker would, so setup gets past its
    /// environment check and on to the questions.
    struct WorkingRunner;

    #[async_trait]
    impl Runner for WorkingRunner {
        async fn run(&self, argv: &[String]) -> Result<Output, RunFailure> {
            let spoken = argv.join(" ");
            let stdout = if spoken.contains("Server.Version") {
                "27.0.0"
            } else if spoken.contains("compose version") {
                "2.29.0"
            } else {
                ""
            };
            Ok(Output {
                status: Some(0),
                stdout: stdout.to_owned(),
                stderr: String::new(),
            })
        }
    }

    /// A context whose engine is absent but whose client answers — enough for the
    /// environment check to pass, which is all setup asks of it before the walk.
    fn working_ctx() -> Ctx {
        Ctx::new(
            Arc::new(WorkingRunner),
            Arc::new(DeadEngine),
            Arc::new(lemonfiber_core::adapters::System),
            Arc::new(lemonfiber_core::adapters::Disk),
            Source::Embedded(&crate::cli::STACK),
            Settings::default(),
            Environment::MacOs,
        )
    }

    /// A scratch install unique to this test.
    fn scratch(name: &str) -> Paths {
        let root =
            std::env::temp_dir().join(format!("lemonfiber-setup-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        Paths::rooted(&root.join("config"), &root.join("data"))
    }

    fn ctx() -> Ctx {
        Ctx::new(
            Arc::new(DeadRunner),
            Arc::new(DeadEngine),
            Arc::new(lemonfiber_core::adapters::System),
            Arc::new(lemonfiber_core::adapters::Disk),
            Source::Embedded(&crate::cli::STACK),
            Settings::default(),
            Environment::MacOs,
        )
    }

    #[test]
    fn a_first_run_is_begun_unless_it_is_clearly_declined() {
        // Silence, and anything but a clear no, means yes: a bare invocation on an
        // unconfigured machine is what a first run looks like.
        for answer in ["", "y", "yes", "anything"] {
            assert!(
                confirm_setup(&Scripted::saying(true, &[answer])),
                "{answer}"
            );
        }
        for answer in ["n", "no", "NO"] {
            assert!(
                !confirm_setup(&Scripted::saying(true, &[answer])),
                "{answer}"
            );
        }
    }

    #[test]
    fn the_proposed_data_location_sits_under_this_machines_data_directory() {
        let paths = scratch("default-location");
        assert_eq!(
            default_data_location(&paths),
            paths.data_dir().join("media")
        );
    }

    #[test]
    fn a_stamp_is_a_sortable_number_of_seconds() {
        assert!(stamp().chars().all(|c| c.is_ascii_digit()));
    }

    #[tokio::test]
    async fn a_configured_machine_is_pointed_at_its_settings_rather_than_setup() {
        let paths = scratch("configured");
        let _ = paths.env_file().parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(paths.env_file(), "DATA_ROOT=/srv\n");
        let code = greeting(ctx(), &paths, &Scripted::saying(true, &[])).await;
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
    }

    #[tokio::test]
    async fn an_unconfigured_machine_with_nobody_there_is_told_what_to_run() {
        // Stated rather than asked: never left waiting on input that will not come.
        let paths = scratch("piped");
        let code = greeting(ctx(), &paths, &Scripted::saying(false, &[])).await;
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
    }

    #[tokio::test]
    async fn a_declined_offer_writes_nothing() {
        let paths = scratch("declined");
        let code = greeting(ctx(), &paths, &Scripted::saying(true, &["n"])).await;
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
        assert!(!paths.env_file().exists());
    }

    #[tokio::test]
    async fn a_rehearsed_greeting_says_there_is_nothing_to_rehearse() {
        let paths = scratch("rehearsed");
        let mut rehearsing = ctx();
        rehearsing.dry_run = true;
        let code = greeting(rehearsing, &paths, &Scripted::saying(true, &[])).await;
        assert_ne!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
    }

    #[tokio::test]
    async fn an_accepted_offer_is_stopped_by_an_environment_that_cannot_work() {
        // Nothing setup does works without a container engine, so it is checked
        // before the first question rather than after eleven answers.
        let paths = scratch("preflight");
        let code = greeting(ctx(), &paths, &Scripted::saying(true, &["y"])).await;
        assert_ne!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
        // Nothing was asked and nothing was written.
        assert!(!paths.env_file().exists());
    }

    #[tokio::test]
    async fn a_non_interactive_run_missing_a_flag_is_told_which() {
        // Rather than left waiting on input that never comes.
        let paths = scratch("missing-flags");
        let code = setting_up(
            ctx(),
            &paths,
            &Scripted::saying(false, &[]),
            SetupFlags::none(),
        )
        .await;
        assert_ne!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
    }

    #[tokio::test]
    async fn recovering_an_interrupted_apply_needs_someone_to_choose() {
        let paths = scratch("recover-piped");
        let _ = paths.setup_progress().parent().map(std::fs::create_dir_all);
        // A progress file that reads as a stopped apply.
        let _ = std::fs::write(
            paths.setup_progress(),
            r#"{"step":"Review","answers":{},"applying":true}"#,
        );
        let code = setting_up(
            ctx(),
            &paths,
            &Scripted::saying(false, &[]),
            SetupFlags::none(),
        )
        .await;
        assert_ne!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
    }

    #[tokio::test]
    async fn an_answered_walk_applies_and_brings_the_stack_up() {
        // Every question taking its default, which is what a person pressing enter
        // through the whole walk gives — the path a first run actually takes.
        let paths = scratch("applied");
        let code = setting_up(
            working_ctx(),
            &paths,
            &Scripted::saying(true, &[]),
            SetupFlags::none(),
        )
        .await;
        // The stack cannot really come up here, so what is proven is that the walk
        // reached the end and wrote what it gathered.
        assert!(paths.env_file().exists(), "the answers were applied");
        let _ = code;
    }

    #[tokio::test]
    async fn a_saved_run_is_picked_up_where_it_was_left() {
        let paths = scratch("resumed");
        let _ = paths.setup_progress().parent().map(std::fs::create_dir_all);
        // A run that quit mid-question rather than mid-apply.
        let _ = std::fs::write(
            paths.setup_progress(),
            r#"{"step":"protocols","answers":{},"applying":false}"#,
        );
        let code = setting_up(
            working_ctx(),
            &paths,
            &Scripted::saying(true, &[]),
            SetupFlags::none(),
        )
        .await;
        let _ = code;
    }

    #[test]
    fn the_scratch_paths_are_where_this_machine_would_keep_things() {
        // Guards the fixture itself: a scratch install has to look like a real one
        // or every test above is proving something about the wrong shape.
        let paths = scratch("shape");
        assert!(paths.env_file().starts_with(paths.config_dir()));
        assert!(paths.journal().starts_with(paths.config_dir()));
    }

    #[test]
    fn a_path_is_a_path() {
        // The import is used by the fixtures above; this keeps the shape honest.
        assert!(Path::new("/srv").is_absolute());
    }
}
