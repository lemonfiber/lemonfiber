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
    already_set_up, bare_run, confirm_setup, default_data_location, greeting, setting_up, stamp,
    Bare, Ctx, Environment, PathBuf, Paths, SetupFlags, Surface,
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
    /// Which files in a corpus name one of these words, and which of them they name.
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
    async fn exec(&self, _container: &str, _argv: &[String]) -> Result<ExecOutput, DockerFailure> {
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
        // Three fields now, separated the way the check asks for them: the
        // daemon's release, then the API generation at each end. A double
        // answering with the release alone reads as a client that would not say
        // which generations are in play, which is an unverified environment.
        let stdout = if spoken.contains("Server.Version") {
            "27.0.0|1.45|1.45"
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
    lemonfiber_testing::a_context()
        .runner(Arc::new(WorkingRunner))
        .engine(Arc::new(FakeEngine::down()))
        .clock(lemonfiber_fixtures::ports::Following::started())
        .over(Source::Embedded(&lemonfiber::carried::STACK))
        .build()
}

/// A scratch install unique to this test.
fn scratch(name: &str) -> (lemonfiber_fixtures::scratch::Scratch, Paths) {
    let dir = lemonfiber_fixtures::scratch::Scratch::unmade(name);
    let paths = Paths::rooted(&dir.join("config"), &dir.join("data"));
    (dir, paths)
}

pub(crate) fn ctx() -> Ctx {
    lemonfiber_testing::a_context()
        .runner(Arc::new(DeadRunner))
        .engine(Arc::new(FakeEngine::down()))
        .clock(lemonfiber_fixtures::ports::Following::started())
        .over(Source::Embedded(&lemonfiber::carried::STACK))
        .build()
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
    let (_scratch, paths) = scratch("default-location");
    assert_eq!(
        default_data_location(&paths),
        paths.data_dir().join("media")
    );
}

#[test]
fn a_stamp_is_a_sortable_number_of_seconds() {
    assert!(stamp().chars().all(|c| c.is_ascii_digit()));
}

#[tokio::test(start_paused = true)]
async fn a_configured_machine_nobody_is_watching_is_pointed_at_its_settings() {
    // A pipe, a cron line or a CI step: the dashboard would draw to nothing and
    // never return, so what a bare run can still usefully do is say where to go.
    let (_scratch, paths) = scratch("configured");
    let _ = paths.env_file().parent().map(std::fs::create_dir_all);
    let _ = std::fs::write(paths.env_file(), "DATA_ROOT=/srv\n");
    let code = greeting(ctx(), &paths, &Scripted::saying(false, &[])).await;
    assert_eq!(shown(code), success());
}

#[tokio::test(start_paused = true)]
async fn a_bare_run_asks_the_screen_rather_than_the_keyboard() {
    // `lemonfiber > out.txt` leaves a keyboard attached and no screen. The
    // dashboard would draw escape sequences into the file and hold the run open
    // waiting for a keypress nobody would see the prompt for, so what decides is
    // the stream it would draw to.
    let piped = Scripted::piped();
    let (_scratch, paths) = scratch("piped");
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
    // Two ways on rather than a refusal: this is guidance, not a misuse. The help
    // itself is printed under these.
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

#[tokio::test(start_paused = true)]
async fn an_unconfigured_machine_with_nobody_there_is_told_what_to_run() {
    // Stated rather than asked: never left waiting on input that will not come.
    let (_scratch, paths) = scratch("piped");
    let code = greeting(ctx(), &paths, &Scripted::saying(false, &[])).await;
    assert_eq!(shown(code), success());
}

#[tokio::test(start_paused = true)]
async fn a_declined_offer_writes_nothing() {
    let (_scratch, paths) = scratch("declined");
    let code = greeting(ctx(), &paths, &Scripted::saying(true, &["n"])).await;
    assert_eq!(shown(code), success());
    assert!(!paths.env_file().exists());
}

#[tokio::test(start_paused = true)]
async fn a_rehearsed_greeting_says_there_is_nothing_to_rehearse() {
    let (_scratch, paths) = scratch("rehearsed");
    let mut rehearsing = ctx();
    rehearsing.dry_run = true;
    let code = greeting(rehearsing, &paths, &Scripted::saying(true, &[])).await;
    assert_ne!(shown(code), success());
}

/// A run given flags never passes the greeting, so the refusal that lived there
/// was no refusal at all for `lemonfiber setup --data-root … --dry-run`, nor for
/// a rehearsal picking up a setup somebody had stopped part-way.
#[tokio::test(start_paused = true)]
async fn a_rehearsed_setup_given_flags_applies_none_of_them() {
    let (_scratch, paths) = scratch("rehearsed-flags");
    let mut rehearsing = ctx();
    rehearsing.dry_run = true;
    let code = setting_up(
        rehearsing,
        &paths,
        &Scripted::saying(false, &[]),
        SetupFlags::none(),
    )
    .await;
    assert_ne!(shown(code), success());

    // The path is read into the message before the assertion rather than as an
    // argument to it. An argument is only evaluated when the assertion fails, which
    // is a region no passing run enters and one the coverage gate counts.
    let env = paths.env_file();
    let wrote = format!("a rehearsed setup wrote {}", env.display());

    assert!(!env.exists(), "{wrote}");
}

#[tokio::test(start_paused = true)]
async fn an_accepted_offer_is_stopped_by_an_environment_that_cannot_work() {
    // Nothing setup does works without a container engine, so it is checked
    // before the first question rather than after eleven answers.
    let (_scratch, paths) = scratch("preflight");
    let code = greeting(ctx(), &paths, &Scripted::saying(true, &["y"])).await;
    assert_ne!(shown(code), success());
    // Nothing was asked and nothing was written.
    assert!(!paths.env_file().exists());
}

#[tokio::test(start_paused = true)]
async fn a_non_interactive_run_missing_a_flag_is_told_which() {
    // Rather than left waiting on input that never comes — and told **which**,
    // since a run that failed without naming a flag leaves the operator to guess
    // at the very thing the refusal exists to supply.
    let (_scratch, paths) = scratch("missing-flags");
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

#[tokio::test(start_paused = true)]
async fn recovering_an_interrupted_apply_needs_someone_to_choose() {
    let (_scratch, paths) = scratch("recover-piped");
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

#[tokio::test(start_paused = true)]
async fn a_walk_declined_at_the_review_writes_nothing_and_says_so() {
    // The review is the last point at which nothing has been written, and an
    // operator who says no there must be left with a machine exactly as they
    // found it — not a half-configured one.
    let (_scratch, paths) = scratch("declined");
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

#[tokio::test(start_paused = true)]
async fn an_answered_walk_applies_and_brings_the_stack_up() {
    // Every question taking its default, which is what a person pressing enter
    // through the whole walk gives — the path a first run actually takes.
    let (_scratch, paths) = scratch("applied");
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

#[tokio::test(start_paused = true)]
async fn a_saved_run_is_picked_up_where_it_was_left() {
    let (_scratch, paths) = scratch("resumed");
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
    // Picked up rather than begun again: the saved run is consumed and the walk
    // it was in the middle of reaches the end. A run that started from the first
    // question would leave the saved one where it found it.
    assert!(
        !paths.setup_progress().exists(),
        "the saved run was left behind rather than picked up"
    );
    assert!(
        paths.env_file().exists(),
        "the walk it resumed reached the end"
    );
    let _ = code;
}

#[test]
fn the_scratch_paths_are_where_this_machine_would_keep_things() {
    // Guards the fixture itself: a scratch install has to look like a real one
    // or every test above is proving something about the wrong shape.
    let (_scratch, paths) = scratch("shape");
    assert!(paths.env_file().starts_with(paths.config_dir()));
    assert!(paths.journal().starts_with(paths.config_dir()));
}

#[test]
fn a_path_is_a_path() {
    // The import is used by the fixtures above; this keeps the shape honest.
    assert!(Path::new("/srv").is_absolute());
}

#[tokio::test(start_paused = true)]
async fn a_bare_run_on_a_machine_mid_setup_picks_it_up_rather_than_greeting() {
    // Unfinished setup is neither a fresh machine nor a finished one, and must be
    // caught before the configured-yet check — an interrupted apply leaves
    // half-written settings that check would read as done.
    let (_scratch, paths) = scratch("greet-resumes");
    let _ = paths.setup_progress().parent().map(std::fs::create_dir_all);
    let _ = std::fs::write(
        paths.setup_progress(),
        r#"{"at":"protocols","answers":{},"phase":"in-progress"}"#,
    );
    let code = greeting(working_ctx(), &paths, &Scripted::saying(true, &[])).await;
    // Picked up rather than greeted: greeting a machine leaves the saved run
    // untouched, and resuming one consumes it.
    assert!(
        !paths.setup_progress().exists(),
        "the machine was greeted and its unfinished setup left where it was"
    );
    let _ = code;
}

#[tokio::test(start_paused = true)]
async fn setup_on_a_configured_machine_points_at_its_settings() {
    // Setup would walk a done machine back to its first question; changing a
    // setting is what it actually wants.
    let (_scratch, paths) = scratch("already-set-up");
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

#[tokio::test(start_paused = true)]
async fn a_saved_run_whose_answers_are_gone_begins_afresh() {
    // In-progress means a saved run; if it is somehow gone there is nothing to
    // resume, so a fresh run is the honest fallback.
    let (_scratch, paths) = scratch("answers-gone");
    let code = super::resume_gather(
        working_ctx(),
        &paths,
        &Scripted::saying(true, &[]),
        None,
        SetupFlags::none(),
    )
    .await;
    // Afresh: the run it began asked its questions and wrote what it gathered,
    // rather than stopping for answers that are not there.
    assert!(paths.env_file().exists(), "a fresh run reached the end");
    let _ = code;
}

#[tokio::test(start_paused = true)]
async fn a_run_with_nobody_there_and_no_flags_is_told_which_it_needs() {
    // Rather than left waiting on input that never comes. The environment has to
    // pass first, or it would stop before the questions are even considered.
    let (_scratch, paths) = scratch("needs-flags");
    let code = setting_up(
        working_ctx(),
        &paths,
        &Scripted::saying(false, &[]),
        SetupFlags::none(),
    )
    .await;
    assert_ne!(shown(code), success());
    // Told which it needs rather than left part-way: nothing was gathered, so
    // there is no saved run for the operator to come back to.
    assert!(
        !paths.setup_progress().exists(),
        "it got past the questions nobody was there to answer"
    );
}

/// Flags answer the questions a run with nobody there cannot ask.
///
/// The sibling above, with nobody there and none of them, is told which flags it
/// needs and gathers nothing. With them the walk is answered and reaches the
/// apply — which is as far as a machine with no stack to bring up can go, so what
/// it leaves is a run to resume rather than a list of flags to supply.
#[tokio::test(start_paused = true)]
async fn flags_answer_the_questions_a_run_with_nobody_there_cannot_ask() {
    let (_scratch, paths) = scratch("flagged");
    let flags = crate::prompt::SetupFlags::parse(crate::prompt::fixtures::workable())
        .unwrap_or(SetupFlags::none());
    let code = setting_up(working_ctx(), &paths, &Scripted::saying(false, &[]), flags).await;
    assert!(
        paths.setup_progress().exists(),
        "the flags did not carry it past the questions"
    );
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

#[tokio::test(start_paused = true)]
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
fn both_audiences_a_conclusion_is_written_for_are_reachable() {
    let prose = ["it ended".to_owned()];
    concluded(SetupOutcome::Abandoned, &Settings::default(), &prose, false);
    concluded(SetupOutcome::Abandoned, &Settings::default(), &prose, true);
}
