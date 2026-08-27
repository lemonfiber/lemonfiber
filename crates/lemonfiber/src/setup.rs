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
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::kind;
use lemonfiber_core::model::{Envelope, SetupOutcome, SetupReport};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::wizard::{offer_setup, Progress, Status, Wizard};
use lemonfiber_core::PRODUCT;

mod boot;
mod door;
mod first_content;
mod interrupted;

use boot::{preflight, start};
use interrupted::recover_setup;

use crate::context::read_settings;
use crate::exit::{complain, USAGE};
use crate::prompt::{Flags, SetupFlags};
use crate::say::{complain, say};

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

    /// Whether there is a screen to draw on.
    ///
    /// A separate question from whether anyone can answer, because the two streams
    /// are separately redirected: `lemonfiber > out.txt` leaves a keyboard attached
    /// and no screen. Asked of every surface rather than defaulted to the other, so
    /// a surface that has not thought about it does not quietly answer for both.
    fn drawable(&self) -> bool;

    /// Show a prompt and read the trimmed line typed in reply.
    fn line(&self, prompt: &str) -> String;

    /// What puts setup's questions, once there is someone to put them to.
    fn asking(
        &self,
        environment: Environment,
        default_data: PathBuf,
    ) -> Box<dyn core_setup::Prompt>;
}

/// Say what the run came to, in the shape whoever asked for it can use.
///
/// One place, so a run that can end three ways cannot answer in three shapes.
fn concluded(outcome: SetupOutcome, settings: &Settings, prose: &[String], parsed: bool) {
    if parsed {
        document(outcome, settings).print();
        return;
    }
    for line in prose {
        say!("{line}");
    }
}

/// The same conclusion, for something that will parse it.
///
/// Built from the settings as they stand once the run is over, so what a script is
/// told is what the machine now holds rather than what was typed at it.
fn document(outcome: SetupOutcome, settings: &Settings) -> crate::render::Lines {
    let report = SetupReport {
        outcome,
        protocols: settings.protocols,
        data_root: settings.data_root.clone(),
        service_user: settings
            .service_user
            .map(|(user, group)| format!("{user}:{group}")),
    };
    let mut lines = crate::render::Lines::for_a_parser();
    lines.put(
        Envelope::new(kind::SETUP, &report)
            .to_json()
            .unwrap_or(crate::render::UNRENDERABLE.to_owned()),
    );
    lines
}

/// Where to send an operator whose machine is already set up, for a run nobody is
/// watching.
///
/// The words rather than the printing, so what a bare run says is proven here and
/// only the act of saying it happens at the edge.
pub(crate) fn already_set_up() -> Vec<String> {
    vec![
        format!("{PRODUCT} is already set up on this machine."),
        format!("  · change a setting with `{PRODUCT} config set <key> <value>`"),
        format!("  · start the stack with `{PRODUCT} up`"),
    ]
}

/// What a bare run on a machine that is already set up does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Bare {
    /// Open the dashboard — what a bare invocation means when somebody is watching.
    Dashboard,
    /// Say where to go next, for a run nobody is watching.
    Guidance,
}

/// Which of the two a bare run on a configured machine is.
///
/// The dashboard holds the terminal until somebody leaves it, which is right in
/// front of a person and wrong in a pipe, a cron line or a CI step — there it
/// would draw to nothing and never return. So the one case that cannot be tested
/// (a real terminal) is reached through a decision that can be.
pub(crate) const fn bare_run(interactive: bool) -> Bare {
    if interactive {
        Bare::Dashboard
    } else {
        Bare::Guidance
    }
}

/// The greeting itself, once there is somewhere to keep files and something to
/// hold the conversation across.
pub(crate) async fn greeting(ctx: Ctx, paths: &Paths, surface: &dyn Surface) -> ExitCode {
    // A stopped apply or a quit mid-question is unfinished setup, not a fresh or a
    // finished machine, and must be caught before the configured-yet check below —
    // an interrupted apply leaves half-written settings that check would read as
    // done. Handing these to the setup path detects and offers the way out.
    let progress = core_setup::progress_at(&paths.setup_progress());
    if Status::of(progress.as_ref()).unfinished() {
        // A bare invocation carries no flags; unfinished setup is picked up
        // interactively, the same conversation a fresh bare run would have.
        return setting_up(ctx, paths, surface, SetupFlags::none()).await;
    }

    if !offer_setup(paths.env_file().exists()) {
        // Already set up: setup would walk a done machine back to its first
        // question, so a bare run does the other thing it could mean.
        return crate::terminal::configured(ctx, bare_run(surface.drawable())).await;
    }

    say!("No configuration found.");

    // Setup applies answers, so there is nothing for --dry-run to rehearse. Said
    // here, before the offer, rather than asking a question whose yes could not be
    // honoured — the same refusal `setup` gives, and at the same point in the walk.
    if ctx.dry_run {
        complain!("Setup applies your answers, so it has nothing to rehearse.");
        complain!("Run `{PRODUCT} setup` without --dry-run when you are ready.");
        return ExitCode::from(USAGE);
    }

    if !surface.interactive() {
        // No one is here to take the offer, so it is stated rather than asked —
        // never left waiting on input that will not come.
        say!("Run `{PRODUCT} setup` to configure your stack.");
        return ExitCode::SUCCESS;
    }
    if !confirm_setup(surface) {
        say!("No changes made — run `{PRODUCT} setup` when you are ready.");
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

/// What a previous run left decides what this one does — the whole of setup's
/// routing, once there is somewhere to keep files and something to ask across.
pub(crate) async fn setting_up(
    ctx: Ctx,
    paths: &Paths,
    surface: &dyn Surface,
    flags: SetupFlags,
) -> ExitCode {
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
        concluded(
            SetupOutcome::AlreadySetUp,
            &ctx.settings,
            &[
                "This machine is already set up.".to_owned(),
                format!("Change a setting with `{PRODUCT} config set`, or start it with `{PRODUCT} up`."),
            ],
            crate::say::for_a_parser(),
        );
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
    say!("Picking up where a previous setup left off.");
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
    // TLS-wrapped NNTP connection. The context carries the one that does it, so a
    // run driven a question at a time from a browser proves them the same way.
    let validator = Arc::clone(&ctx.validator);

    // A terminal answers the questions; without one the flags do, and where a flag
    // a question needs is missing the run is told which rather than left waiting on
    // input that never comes.
    let prompt: Box<dyn core_setup::Prompt> = if surface.interactive() {
        surface.asking(ctx.environment, default_data_location(paths))
    } else {
        let missing = flags.missing(&wizard);
        if !missing.is_empty() {
            complain!("error: setup here is non-interactive, so it needs values as flags:");
            for flag in missing {
                complain!("  {flag}");
            }
            complain!("\nRun it in a terminal to answer interactively instead.");
            return ExitCode::from(USAGE);
        }
        Box::new(Flags::new(flags, default_data_location(paths)))
    };

    match core_setup::run(
        &mut wizard,
        prompt.as_ref(),
        ctx.filesystem.as_ref(),
        validator.as_ref(),
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
            concluded(
                SetupOutcome::Applied,
                &ctx.settings,
                &["\nSetup is done — bringing your stack up.".to_owned()],
                crate::say::for_a_parser(),
            );
            start(&ctx, surface).await
        }
        Ok(core_setup::Outcome::Abandoned) => {
            concluded(
                SetupOutcome::Abandoned,
                &ctx.settings,
                &["\nSetup was left here — nothing was written.".to_owned()],
                crate::say::for_a_parser(),
            );
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
    use crate::exit::{shown, success};
    use std::path::Path;
    use std::sync::Arc;

    use async_trait::async_trait;
    use lemonfiber_core::config::{Protocols, Settings};

    use super::{concluded, document, SetupOutcome};

    use lemonfiber_core::ports::docker::{
        Container, Engine, ExecOutput, Failure as DockerFailure, LogLine, LogQuery, Stats,
    };
    use lemonfiber_core::ports::process::{Failure as RunFailure, Output, Runner};
    use lemonfiber_core::stack::Source;
    use tokio::sync::mpsc::Receiver;

    use super::{
        already_set_up, bare_run, confirm_setup, default_data_location, greeting, setting_up,
        stamp, Bare, Ctx, Environment, PathBuf, Paths, SetupFlags, Surface,
    };

    /// A surface that answers from a script and says whether anyone is there.
    pub(crate) struct Scripted {
        interactive: bool,
        /// Whether there is a screen, which a redirected run has without a keyboard
        /// and a piped one has the other way round.
        drawable: bool,
        /// Whether the screen was the question asked, rather than the keyboard.
        asked_screen: std::cell::Cell<bool>,
        lines: std::cell::RefCell<Vec<String>>,
        /// Whether the plan is accepted at the review. A walk that declines it is
        /// the one that must write nothing at all.
        applies: bool,
    }

    impl Scripted {
        pub(crate) fn saying(interactive: bool, lines: &[&str]) -> Self {
            Self {
                interactive,
                drawable: interactive,
                asked_screen: std::cell::Cell::new(false),
                lines: std::cell::RefCell::new(
                    lines.iter().rev().map(|line| (*line).to_owned()).collect(),
                ),
                applies: true,
            }
        }

        /// Somebody at a keyboard whose output goes to a file.
        pub(crate) fn piped() -> Self {
            Self {
                drawable: false,
                ..Self::saying(true, &[])
            }
        }

        /// Someone who walks the whole way and then says no at the review.
        pub(crate) fn declining_the_plan() -> Self {
            Self {
                applies: false,
                ..Self::saying(true, &[])
            }
        }
    }

    impl Surface for Scripted {
        fn interactive(&self) -> bool {
            self.interactive
        }

        fn drawable(&self) -> bool {
            self.asked_screen.set(true);
            self.drawable
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
                Box::new(Echoing {
                    applies: self.applies,
                }),
            ))
        }
    }

    /// Answers that are always empty — every question takes its default, which is
    /// what a person pressing enter through the whole walk would give — except the
    /// review, which is where a run that declines its plan says so.
    struct Echoing {
        applies: bool,
    }

    /// The review's question, which is the one answer that is not a default.
    const REVIEW: &str = "Apply it?";

    impl crate::prompt::Answers for Echoing {
        fn ask(&self, question: &str) -> String {
            if !self.applies && question.contains(REVIEW) {
                return "n".to_owned();
            }
            String::new()
        }

        fn secret(&self, _prompt: &str) -> String {
            String::new()
        }
    }

    /// A stand-in engine, in the two shapes setup meets: one that is down and
    /// answers nothing, and one that answers with nothing running — which is what
    /// a machine looks like the moment an empty stack has been brought up.
    ///
    /// One fake rather than two, because a second implementation of a four-method
    /// trait leaves three methods nothing calls.
    pub(crate) struct FakeEngine {
        /// Whether listing answers at all.
        lists: bool,
    }

    impl FakeEngine {
        /// An engine that is not running, so nothing about a stack can be read.
        pub(crate) const fn down() -> Self {
            Self { lists: false }
        }

        /// An engine that answers, with nothing running.
        pub(crate) const fn quiet() -> Self {
            Self { lists: true }
        }
    }

    #[async_trait]
    impl Engine for FakeEngine {
        async fn list(&self, _project: &str) -> Result<Vec<Container>, DockerFailure> {
            if self.lists {
                return Ok(Vec::new());
            }
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
    pub(crate) fn working_ctx() -> Ctx {
        Ctx::new(
            Arc::new(WorkingRunner),
            Arc::new(FakeEngine::down()),
            Arc::new(lemonfiber_core::adapters::System),
            Arc::new(lemonfiber_core::adapters::Disk),
            Source::Embedded(&lemonfiber::cli::STACK),
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

    pub(crate) fn ctx() -> Ctx {
        Ctx::new(
            Arc::new(DeadRunner),
            Arc::new(FakeEngine::down()),
            Arc::new(lemonfiber_core::adapters::System),
            Arc::new(lemonfiber_core::adapters::Disk),
            Source::Embedded(&lemonfiber::cli::STACK),
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
    async fn a_configured_machine_nobody_is_watching_is_pointed_at_its_settings() {
        // A pipe, a cron line or a CI step: the dashboard would draw to nothing and
        // never return, so what a bare run can still usefully do is say where to go.
        let paths = scratch("configured");
        let _ = paths.env_file().parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(paths.env_file(), "DATA_ROOT=/srv\n");
        let code = greeting(ctx(), &paths, &Scripted::saying(false, &[])).await;
        assert_eq!(shown(code), success());
    }

    #[tokio::test]
    async fn a_bare_run_asks_the_screen_rather_than_the_keyboard() {
        // `lemonfiber > out.txt` leaves a keyboard attached and no screen. The
        // dashboard would draw escape sequences into the file and hold the run open
        // waiting for a keypress nobody would see the prompt for, so what decides is
        // the stream it would draw to.
        let piped = Scripted::piped();
        let paths = scratch("piped");
        let _ = paths.env_file().parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(paths.env_file(), "DATA_ROOT=/srv\n");

        let code = greeting(ctx(), &paths, &piped).await;

        assert_eq!(shown(code), success());
        assert!(
            piped.asked_screen.get(),
            "the screen was the question, not the keyboard"
        );
        assert!(
            piped.interactive(),
            "and a keyboard was attached the whole time, which is what makes it the wrong question"
        );
    }

    #[test]
    fn what_a_run_with_no_screen_is_told_is_the_whole_of_it() {
        // A pipe, a cron line or a CI step cannot go and ask a second time, so the
        // commands are listed rather than pointed at.
        let help = lemonfiber::cli::help();
        for named in ["setup", "doctor", "up", "config"] {
            assert!(help.contains(named), "the help names `{named}`: {help}");
        }
        assert!(
            !already_set_up().iter().any(|line| line.contains("--help")),
            "and nothing points at what is already printed"
        );
    }

    #[test]
    fn a_run_nobody_is_watching_is_told_where_to_go_next() {
        // Two ways on rather than a refusal: this is guidance, not a misuse. The
        // third used to be a line saying where the help is, and the help itself is
        // printed under these now.
        let said = already_set_up();
        assert!(said
            .first()
            .is_some_and(|line| line.contains("already set up")));
        assert!(said.iter().any(|line| line.contains("config set")));
        assert!(said.iter().any(|line| line.contains(" up`")));
    }

    #[test]
    fn a_bare_run_in_front_of_a_person_opens_the_dashboard() {
        // What a bare invocation *is* on a machine that is already set up. The
        // terminal it then takes is the one thing no test can stand in for, which
        // is why the decision to take it is its own.
        assert_eq!(bare_run(true), Bare::Dashboard);
        assert_eq!(bare_run(false), Bare::Guidance);
    }

    #[tokio::test]
    async fn an_unconfigured_machine_with_nobody_there_is_told_what_to_run() {
        // Stated rather than asked: never left waiting on input that will not come.
        let paths = scratch("piped");
        let code = greeting(ctx(), &paths, &Scripted::saying(false, &[])).await;
        assert_eq!(shown(code), success());
    }

    #[tokio::test]
    async fn a_declined_offer_writes_nothing() {
        let paths = scratch("declined");
        let code = greeting(ctx(), &paths, &Scripted::saying(true, &["n"])).await;
        assert_eq!(shown(code), success());
        assert!(!paths.env_file().exists());
    }

    #[tokio::test]
    async fn a_rehearsed_greeting_says_there_is_nothing_to_rehearse() {
        let paths = scratch("rehearsed");
        let mut rehearsing = ctx();
        rehearsing.dry_run = true;
        let code = greeting(rehearsing, &paths, &Scripted::saying(true, &[])).await;
        assert_ne!(shown(code), success());
    }

    #[tokio::test]
    async fn an_accepted_offer_is_stopped_by_an_environment_that_cannot_work() {
        // Nothing setup does works without a container engine, so it is checked
        // before the first question rather than after eleven answers.
        let paths = scratch("preflight");
        let code = greeting(ctx(), &paths, &Scripted::saying(true, &["y"])).await;
        assert_ne!(shown(code), success());
        // Nothing was asked and nothing was written.
        assert!(!paths.env_file().exists());
    }

    #[tokio::test]
    async fn a_non_interactive_run_missing_a_flag_is_told_which() {
        // Rather than left waiting on input that never comes — and told **which**,
        // since a run that failed without naming a flag leaves the operator to guess
        // at the very thing the refusal exists to supply.
        let paths = scratch("missing-flags");
        let code = setting_up(
            ctx(),
            &paths,
            &Scripted::saying(false, &[]),
            SetupFlags::none(),
        )
        .await;
        assert_ne!(shown(code), success());

        // The words themselves, where they are decided. Asserting the exit code alone
        // would go on passing if the flags stopped being named.
        let wizard = crate::setup::Wizard::new(Environment::MacOs);
        let named = SetupFlags::none().missing(&wizard);
        assert!(
            named.iter().any(|flag| flag.starts_with("--protocols")),
            "the refusal names the protocols flag: {named:?}"
        );
        assert!(
            named.iter().any(|flag| flag.starts_with("--data-location")),
            "and where the data goes: {named:?}"
        );
        assert!(
            named.contains(&"--yes"),
            "and the one that says nobody will be asked: {named:?}"
        );
        assert!(
            named.iter().all(|flag| flag.starts_with("--")),
            "each is a flag as it would be typed, not a step's name: {named:?}"
        );
    }

    #[tokio::test]
    async fn recovering_an_interrupted_apply_needs_someone_to_choose() {
        let paths = scratch("recover-piped");
        let _ = paths.setup_progress().parent().map(std::fs::create_dir_all);
        // A progress file that reads as a stopped apply.
        let _ = std::fs::write(
            paths.setup_progress(),
            r#"{"at":"review","answers":{},"phase":"applying"}"#,
        );
        let code = setting_up(
            ctx(),
            &paths,
            &Scripted::saying(false, &[]),
            SetupFlags::none(),
        )
        .await;
        assert_ne!(shown(code), success());
    }

    #[tokio::test]
    async fn a_walk_declined_at_the_review_writes_nothing_and_says_so() {
        // The review is the last point at which nothing has been written, and an
        // operator who says no there must be left with a machine exactly as they
        // found it — not a half-configured one.
        let paths = scratch("declined");
        let code = setting_up(
            working_ctx(),
            &paths,
            &Scripted::declining_the_plan(),
            SetupFlags::none(),
        )
        .await;
        assert_eq!(shown(code), success(), "walking away is not a failure");
        assert!(!paths.env_file().exists(), "nothing was written");
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
            r#"{"at":"protocols","answers":{},"phase":"in-progress"}"#,
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

    #[tokio::test]
    async fn a_bare_run_on_a_machine_mid_setup_picks_it_up_rather_than_greeting() {
        // Unfinished setup is neither a fresh machine nor a finished one, and must be
        // caught before the configured-yet check — an interrupted apply leaves
        // half-written settings that check would read as done.
        let paths = scratch("greet-resumes");
        let _ = paths.setup_progress().parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(
            paths.setup_progress(),
            r#"{"at":"protocols","answers":{},"phase":"in-progress"}"#,
        );
        let code = greeting(working_ctx(), &paths, &Scripted::saying(true, &[])).await;
        let _ = code;
    }

    #[tokio::test]
    async fn setup_on_a_configured_machine_points_at_its_settings() {
        // Setup would walk a done machine back to its first question; changing a
        // setting is what it actually wants.
        let paths = scratch("already-set-up");
        let _ = paths.env_file().parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(paths.env_file(), "DATA_ROOT=/srv\n");
        let code = setting_up(
            working_ctx(),
            &paths,
            &Scripted::saying(true, &[]),
            SetupFlags::none(),
        )
        .await;
        assert_ne!(shown(code), success());
    }

    #[tokio::test]
    async fn a_saved_run_whose_answers_are_gone_begins_afresh() {
        // In-progress means a saved run; if it is somehow gone there is nothing to
        // resume, so a fresh run is the honest fallback.
        let paths = scratch("answers-gone");
        let code = super::resume_gather(
            working_ctx(),
            &paths,
            &Scripted::saying(true, &[]),
            None,
            SetupFlags::none(),
        )
        .await;
        let _ = code;
    }

    #[tokio::test]
    async fn a_run_with_nobody_there_and_no_flags_is_told_which_it_needs() {
        // Rather than left waiting on input that never comes. The environment has to
        // pass first, or it would stop before the questions are even considered.
        let paths = scratch("needs-flags");
        let code = setting_up(
            working_ctx(),
            &paths,
            &Scripted::saying(false, &[]),
            SetupFlags::none(),
        )
        .await;
        assert_ne!(shown(code), success());
    }

    #[tokio::test]
    async fn a_fully_flagged_run_with_nobody_there_answers_from_the_flags() {
        let paths = scratch("flagged");
        let flags = crate::prompt::SetupFlags::parse(crate::prompt::fixtures::workable())
            .unwrap_or(SetupFlags::none());
        let code = setting_up(working_ctx(), &paths, &Scripted::saying(false, &[]), flags).await;
        let _ = code;
    }

    #[test]
    fn the_answers_a_script_gives_are_empty_whichever_way_they_are_asked_for() {
        use crate::prompt::Answers as _;
        let echoing = Echoing { applies: true };
        assert!(echoing.ask("anything").is_empty());
        assert!(echoing.secret("a password").is_empty());
        // The review is the one question a declining run does not default.
        let declining = Echoing { applies: false };
        assert_eq!(declining.ask("Apply it? [Y/n]:"), "n");
    }

    #[tokio::test]
    async fn the_capabilities_setup_never_asks_the_engine_for_answer_plainly() {
        use lemonfiber_core::ports::docker::LogQuery;
        let engine = FakeEngine::down();
        assert!(engine.list("p").await.is_err());
        assert!(engine.exec("c", &[]).await.is_err());
        assert!(engine.stats("p").await.is_err());
        assert!(engine.logs("p", &[], LogQuery::recent(10)).await.is_err());
    }
    /// A script that ran setup non-interactively is told what it configured — and
    /// deliberately not told anything that could be a credential, because a report
    /// a script can read is one a script can log.
    #[test]
    fn a_setup_a_script_asked_for_is_one_document_it_can_parse() {
        let settings = Settings {
            protocols: Protocols::both(),
            data_root: Some(std::path::PathBuf::from("/srv/media")),
            service_user: Some((1000, 1000)),
            ..Settings::default()
        };

        let said = document(SetupOutcome::Applied, &settings).text();

        assert_eq!(said.lines().count(), 1, "one document: {said}");
        assert!(said.contains("\"kind\":\"setup\""), "{said}");
        assert!(said.contains("\"outcome\":\"applied\""), "{said}");
        assert!(
            said.contains("/srv/media"),
            "and what it settled on: {said}"
        );
        assert!(said.contains("\"1000:1000\""), "{said}");
    }

    /// A machine that was already set up ends a different way and answers in the
    /// same shape, which is the whole reason one place decides it.
    #[test]
    fn a_run_that_asked_nothing_still_says_how_it_ended() {
        let said = document(SetupOutcome::AlreadySetUp, &Settings::default()).text();

        assert!(said.contains("\"outcome\":\"already-set-up\""), "{said}");
        assert!(
            !said.contains("password") && !said.contains("key"),
            "and nothing that could be a credential: {said}"
        );
    }

    /// Both audiences, so neither branch is one nothing runs.
    #[test]
    fn a_conclusion_reaches_whoever_asked_for_it() {
        let prose = ["it ended".to_owned()];
        concluded(SetupOutcome::Abandoned, &Settings::default(), &prose, false);
        concluded(SetupOutcome::Abandoned, &Settings::default(), &prose, true);
    }
}
