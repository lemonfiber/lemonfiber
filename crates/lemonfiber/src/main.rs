//! The `lemonfiber` binary — the surfaces, over one core.
//!
//! This crate is the only one that renders. It turns input into a command,
//! hands it to the core, and renders what comes back; it makes no decisions of
//! its own, which is why the same request behaves identically whether it arrived
//! as a subcommand, a keypress or an HTTP route.

use std::io::IsTerminal as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use include_dir::{include_dir, Dir};
use lemonfiber_core::adapters::{Daemon, Disk, Local, System};
use lemonfiber_core::app::{
    dispatch, logs, recover, setup, supervise, Command, Ctx, Outcome, WATCH,
};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::{
    data_root_from_env, ip_echo_from_env, port_forward_from_env, service_user_from_env, store,
    Protocols, Settings,
};
use lemonfiber_core::doctor::{Category, Overall};
use lemonfiber_core::error::Problem;
use lemonfiber_core::journal::{Change, Kind};
use lemonfiber_core::model::Envelope;
use lemonfiber_core::platform::{Environment, HOST_OS};
use lemonfiber_core::ports::docker::LogQuery;
use lemonfiber_core::stack::Source;
use lemonfiber_core::wizard::{
    offer_setup, Choice, Progress, Recovery, Resolution, Status, Wizard,
};
use lemonfiber_core::PRODUCT;

mod prompt;
mod render;
use prompt::Terminal;
use render::{render, watched};

/// The stack this binary carries.
///
/// Embedding it means the common install has one thing to fetch rather than
/// two, and `build.rs` has already refused to produce this binary if the
/// manifest is one it could not read.
static STACK: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");

/// Set up and run your media stack.
#[derive(Debug, Parser)]
#[command(name = "lemonfiber", version, about)]
struct Cli {
    /// Print machine-readable output.
    #[arg(long, global = true)]
    json: bool,

    /// Say what would happen, and change nothing.
    #[arg(long, global = true)]
    dry_run: bool,

    /// Operate a stack directory of your own instead of the built-in one.
    #[arg(long, global = true, value_name = "PATH")]
    stack_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Request>,
}

/// What the operator asked for.
#[derive(Debug, Subcommand)]
enum Request {
    /// Set up the stack by answering a few questions.
    Setup,
    /// Report the versions in play.
    Version,
    /// Start a form, or the union of several.
    Up {
        /// The forms to start.
        #[arg(required = true)]
        forms: Vec<String>,
    },
    /// Stop and remove what a form started.
    Down {
        /// The forms to stop.
        #[arg(required = true)]
        forms: Vec<String>,
    },
    /// Restart services without touching the rest.
    Restart {
        /// The form holding them.
        form: String,
        /// The services to restart; none restarts the whole form.
        services: Vec<String>,
    },
    /// Fetch newer images without applying them.
    Pull {
        /// The forms whose images to fetch.
        #[arg(required = true)]
        forms: Vec<String>,
    },
    /// Report what each service is actually doing.
    Ps {
        /// The forms to report on; none reports the whole stack.
        forms: Vec<String>,
    },
    /// Show what services are saying.
    Logs {
        /// The services to read; none reads them all.
        services: Vec<String>,
        /// Read only the services a form declares.
        #[arg(long, value_name = "FORM")]
        form: Vec<String>,
        /// Keep reading as new lines arrive.
        #[arg(long, short)]
        follow: bool,
        /// How many existing lines to begin with.
        #[arg(long, default_value_t = 50)]
        tail: u32,
    },
    /// Read or change one setting.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Run the checks that prove the stack is doing what it should.
    Doctor {
        /// Run only one category of check, such as `vpn`.
        #[arg(long, value_name = "CATEGORY")]
        only: Option<String>,
        /// Include the checks that disturb the running system.
        #[arg(long)]
        disruptive: bool,
    },
    /// Guard the data location while forms run, stopping them if it disappears.
    Watch {
        /// The forms to stop if the data location is lost.
        #[arg(required = true)]
        forms: Vec<String>,
    },
    /// Wire the stack's services to each other, idempotently.
    Seed,
}

/// What to do with settings.
#[derive(Debug, Subcommand)]
enum ConfigAction {
    /// Read one setting.
    Get {
        /// The setting to read.
        key: String,
    },
    /// Change one setting.
    Set {
        /// The setting to change.
        key: String,
        /// What to change it to.
        value: String,
    },
    /// Show every setting, with credentials withheld.
    Show,
}

/// A general failure. Codes are meaningful so a script can branch on *why*
/// something failed rather than merely on whether it did.
const FAILURE: u8 = 1;

/// A flag or argument the operator gave could not be understood.
const USAGE: u8 = 2;

/// Something outside lemonfiber has to be fixed before it can act.
const PREFLIGHT: u8 = 3;

/// Started, and a service never became usable.
const NEVER_SETTLED: u8 = 4;

/// Something the operator wrote was refused.
const VALIDATION: u8 = 5;

/// Which exit code a problem deserves.
///
/// A script branching on failure needs to know whether to fix its own input,
/// start Docker, or wait longer, and one code for all three tells it nothing.
fn exit_code(problem: &Problem) -> u8 {
    use lemonfiber_core::{app, config, ports, stack};

    match problem.code {
        app::NEVER_SETTLED => NEVER_SETTLED,
        ports::process::MISSING_PROGRAM | ports::docker::ENGINE_UNREACHABLE => PREFLIGHT,
        stack::STACK_INVALID | stack::STACK_UNREADABLE | config::store::CONFIG_UNREADABLE => {
            VALIDATION
        }
        _ => FAILURE,
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let mut cli = Cli::parse();

    let Some(request) = cli.command else {
        return greet(cli.stack_dir.take(), cli.dry_run).await;
    };

    let ctx = context(cli.stack_dir.take(), cli.dry_run);

    let command = match request {
        // Streaming is not a value that arrives once, so it does not become a
        // command and does not go through dispatch. It still goes through the
        // core, which is the part that matters.
        Request::Logs {
            services,
            form,
            follow,
            tail,
        } => return stream(&ctx, &form, &services, follow, tail, cli.json).await,
        // A watch is long-running and produces one report at its end, not a value
        // that arrives once, so like streaming it does not go through dispatch.
        Request::Watch { forms } => return guard(&ctx, &forms, cli.json).await,
        // Setup is a conversation and then a stack coming up, not a value that
        // arrives once, so like streaming and watching it runs its own way. It
        // takes the context by value because it rewrites the settings mid-run.
        Request::Setup => return run_setup(ctx).await,
        Request::Version => Command::Version,
        Request::Up { forms } => Command::Up { forms },
        Request::Down { forms } => Command::Down { forms },
        Request::Restart { form, services } => Command::Restart {
            forms: vec![form],
            services,
        },
        Request::Pull { forms } => Command::Pull { forms },
        Request::Ps { forms } => Command::Ps { forms },
        Request::Config { action } => configuration(action),
        Request::Doctor { only, disruptive } => {
            let only = match only.as_deref().map(Category::parse) {
                // A named category that lemonfiber does not know is a mistake to
                // name, not a request to run everything.
                Some(None) => {
                    let named = only.unwrap_or_default();
                    eprintln!("error: no diagnostic category named `{named}`");
                    return ExitCode::from(USAGE);
                }
                Some(Some(category)) => Some(category),
                None => None,
            };
            Command::Doctor { only, disruptive }
        }
        Request::Seed => Command::Seed,
    };

    match dispatch(command, &ctx).await {
        Ok(outcome) => {
            render(&outcome, cli.json);
            settled(&outcome)
        }
        Err(problem) => complain(&problem),
    }
}

/// The exit code an outcome deserves.
///
/// Most answers are simply produced, so their success is that they arrived. A
/// diagnosis is different: a script runs it precisely to learn whether the stack
/// is healthy, so a broken or undetermined result must exit non-zero — reporting
/// success when nothing could be verified is the falsehood this product exists to
/// avoid.
fn settled(outcome: &Outcome) -> ExitCode {
    match outcome {
        Outcome::Doctor(report) => match report.overall {
            Overall::Healthy | Overall::Degraded => ExitCode::SUCCESS,
            Overall::Broken | Overall::Unknown => ExitCode::from(FAILURE),
        },
        // Seeding is run to make the wiring true, so leaving any of it unmade is a
        // non-zero result a script can act on by running again.
        Outcome::Seed(report) => {
            if report.is_complete() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(FAILURE)
            }
        }
        Outcome::Version(_) | Outcome::Lifecycle(_) | Outcome::Config(_) | Outcome::Status(_) => {
            ExitCode::SUCCESS
        }
    }
}

/// Everything a command needs that the command itself does not carry.
fn context(stack_dir: Option<PathBuf>, dry_run: bool) -> Ctx {
    // The path outlives the process, and `Source` is Copy so it can be handed
    // around freely; leaking one allocation at startup buys both.
    let stack = match stack_dir {
        Some(path) => Source::External(Box::leak(path.into_boxed_path())),
        None => Source::Embedded(&STACK),
    };

    let settings = read_settings();

    // Docker Engine and Docker Desktop are told apart by asking the daemon,
    // which needs the engine adapter. Until then this is what can be seen from
    // here, and nothing yet depends on the difference.
    let environment = Environment::resolve(HOST_OS, false);

    let ctx = Ctx::new(
        Arc::new(Local),
        Arc::new(Daemon::local()),
        Arc::new(System),
        Arc::new(Disk),
        stack,
        settings,
        environment,
    );

    if dry_run {
        return ctx.rehearsing();
    }
    ctx
}

/// The operator's settings, read from their file as it stands now.
///
/// Read fresh rather than passed around, because setup writes the file mid-run:
/// the settings this process started with predate what it just applied, and
/// starting the stack against the stale set would run the wrong thing.
fn read_settings() -> Settings {
    let env_file = configuration_file();
    let recorded = env_file
        .as_deref()
        .and_then(|path| store::read(path).ok())
        .unwrap_or_default();

    Settings {
        protocols: Protocols::from_env(&recorded),
        ip_echo: ip_echo_from_env(&recorded),
        data_root: data_root_from_env(&recorded),
        storage_state: here().map(|paths| paths.storage_state()),
        service_user: service_user_from_env(&recorded),
        port_forward: port_forward_from_env(&recorded),
        env_file,
        stack_dir: stack_directory(),
        ..Settings::default()
    }
}

/// Which setting the operator is reading or changing.
fn configuration(action: ConfigAction) -> Command {
    match action {
        ConfigAction::Get { key } => Command::ConfigGet { key },
        ConfigAction::Set { key, value } => Command::ConfigSet { key, value },
        ConfigAction::Show => Command::ConfigShow,
    }
}

/// Tell the operator what went wrong, and exit in a way a script can branch on.
///
/// One renderer, so a failure reads the same whichever command produced it —
/// the remedies are the point of the error model, and a second copy of this is
/// how one of them quietly starts omitting them.
fn complain(problem: &Problem) -> ExitCode {
    eprintln!("{}: {}", problem.code, problem.summary);
    eprintln!("\n  {}\n", problem.meaning);

    for remedy in &problem.remedies {
        eprintln!("  → {}", remedy.action);
        if let Some(detail) = &remedy.detail {
            eprintln!("    {detail}");
        }
    }

    // Last, and indented: available to whoever wants it, and never the first
    // thing the operator has to read.
    if let Some(detail) = &problem.detail {
        eprintln!();
        for line in detail.lines() {
            eprintln!("  {line}");
        }
    }

    ExitCode::from(exit_code(problem))
}

/// Print log lines as they arrive, until the stream ends.
///
/// Machine-readable output is one envelope per line rather than one document
/// containing all of them: a stream has no last element to close a document
/// with, and a consumer of `--follow --json` needs each line when it happens
/// rather than when the service stops.
async fn stream(
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
            match Envelope::new("log", &line).to_json() {
                Some(text) => println!("{text}"),
                None => eprintln!("this line could not be rendered as JSON"),
            }
        } else {
            println!("{:<12} {}", line.service, line.line);
        }
    }

    // Silence and "no output" are different answers, and a viewer that renders
    // them identically leaves the operator wondering which one they got.
    if seen == 0 && !json {
        println!("no output");
    }
    ExitCode::SUCCESS
}

/// Watch the data location until it is lost, then report what was stopped.
///
/// This blocks for as long as the location holds — the operator ends it with the
/// same interrupt they end any foreground command. It returns only once the
/// location is lost and the services have been stopped, which is the one thing
/// it exists to do.
async fn guard(ctx: &Ctx, forms: &[String], json: bool) -> ExitCode {
    match supervise(ctx, &Disk, forms, WATCH).await {
        Ok(report) => {
            watched(&report, json);
            ExitCode::SUCCESS
        }
        Err(problem) => complain(&problem),
    }
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
async fn greet(stack_dir: Option<PathBuf>, dry_run: bool) -> ExitCode {
    let ctx = context(stack_dir, dry_run);

    let Some(paths) = here() else {
        // With nowhere to keep its files there is nothing to offer and nothing to
        // point at, so the plain pointer is the only honest thing left to say.
        println!("{PRODUCT} — run `{PRODUCT} --help` to see what it can do");
        return ExitCode::SUCCESS;
    };

    // A stopped apply or a quit mid-question is unfinished setup, not a fresh or a
    // finished machine, and must be caught before the configured-yet check below —
    // an interrupted apply leaves half-written settings that check would read as
    // done. Handing these to the setup path detects and offers the way out.
    let progress = setup::progress_at(&paths.setup_progress());
    if matches!(
        Status::of(progress.as_ref()),
        Status::FailedApply | Status::InProgress
    ) {
        return run_setup(ctx).await;
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

    if !std::io::stdin().is_terminal() {
        // No one is here to take the offer, so it is stated rather than asked —
        // never left waiting on input that will not come.
        println!("Run `{PRODUCT} setup` to configure your stack.");
        return ExitCode::SUCCESS;
    }
    if !confirm_setup() {
        println!("No changes made — run `{PRODUCT} setup` when you are ready.");
        return ExitCode::SUCCESS;
    }
    run_setup(ctx).await
}

/// Ask whether to begin setup now, taking silence and anything but a clear no as
/// yes — a first run is what a bare invocation on an unconfigured machine means,
/// so the gentle default is to proceed.
fn confirm_setup() -> bool {
    !matches!(
        read_line("Run first-time setup? [Y/n]")
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
/// and what to write are all the core's, reached through [`setup::run`].
async fn run_setup(ctx: Ctx) -> ExitCode {
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

    // What a previous run left decides what this one does. An apply that stopped
    // part-way is offered back before anything else — otherwise the configured-yet
    // check below would see its half-written settings and call the machine done.
    let progress = setup::progress_at(&paths.setup_progress());
    match Status::of(progress.as_ref()) {
        Status::FailedApply => recover_setup(ctx, &paths, progress).await,
        // A run that quit mid-question saved where it reached; pick it back up.
        Status::InProgress => resume_gather(ctx, &paths, progress).await,
        // Absent and applied are neither a stopped apply nor a saved run, so they
        // begin, or decline, a fresh one.
        _ => fresh_setup(ctx, &paths).await,
    }
}

/// Gather answers on a machine with nothing to recover or resume.
async fn fresh_setup(ctx: Ctx, paths: &Paths) -> ExitCode {
    // Setup is for a machine with nothing configured; a configured one is changed
    // through its settings, not walked back to its first question.
    if !offer_setup(paths.env_file().exists()) {
        println!("This machine is already set up.");
        println!("Change a setting with `{PRODUCT} config set`, or start it with `{PRODUCT} up`.");
        return ExitCode::from(USAGE);
    }

    let environment = ctx.environment;
    drive(ctx, paths, Wizard::new(environment)).await
}

/// Pick a setup back up from the answers a quit run saved.
async fn resume_gather(ctx: Ctx, paths: &Paths, progress: Option<Progress>) -> ExitCode {
    // In-progress means a saved run; if it is somehow gone there is nothing to
    // resume, so a fresh run is the honest fallback.
    let Some(progress) = progress else {
        return fresh_setup(ctx, paths).await;
    };
    println!("Picking up where a previous setup left off.");
    let environment = ctx.environment;
    drive(ctx, paths, Wizard::resume(environment, progress)).await
}

/// Ask the questions the `wizard` still needs, apply the answers, and start.
async fn drive(mut ctx: Ctx, paths: &Paths, mut wizard: Wizard) -> ExitCode {
    // The questions need someone at a terminal to answer them. A piped or scripted
    // run has no one, so it is told what it would have been asked rather than left
    // waiting on input that never comes.
    // The environment is checked before the first question, so a missing or
    // unreachable container engine is caught here rather than after eleven
    // answers — nothing setup does can work without one.
    if let Err(code) = preflight(&ctx).await {
        return code;
    }

    // The questions need someone at a terminal to answer them. A piped or scripted
    // run has no one, so it is told what it would have been asked rather than left
    // waiting on input that never comes.
    if !std::io::stdin().is_terminal() {
        eprintln!("error: setup asks questions that need a terminal to answer.");
        eprintln!(
            "Run it interactively, or set values with `{PRODUCT} config set` and start with `{PRODUCT} up`."
        );
        return ExitCode::from(USAGE);
    }

    let prompt = Terminal::new(ctx.environment, default_data_location());

    match setup::run(
        &mut wizard,
        &prompt,
        ctx.filesystem.as_ref(),
        paths,
        ctx.stack,
        &stamp(),
    )
    .await
    {
        Ok(setup::Outcome::Applied) => {
            // The settings read at startup predate the file setup just wrote, so
            // they are refreshed before the stack is brought up against them.
            ctx.settings = read_settings();
            println!("\nSetup is done — bringing your stack up.");
            start(&ctx).await
        }
        Ok(setup::Outcome::Abandoned) => {
            println!("\nSetup was left here — nothing was written.");
            ExitCode::SUCCESS
        }
        Err(problem) => complain(&problem),
    }
}

/// Offer the operator a way out of a setup whose apply stopped part-way.
///
/// It is shown what the interrupted run wrote and given the three ways forward the
/// wizard keeps recoverable: finish it, undo and redo it, or undo and forget it.
/// Deciding is not done for a piped run that cannot answer — the state is left as
/// it is, still recoverable, rather than acted on unasked.
async fn recover_setup(ctx: Ctx, paths: &Paths, progress: Option<Progress>) -> ExitCode {
    // A stopped apply always leaves its answers; if they are somehow gone there is
    // nothing to resume from, so a fresh run is the honest fallback.
    let Some(progress) = progress else {
        return fresh_setup(ctx, paths).await;
    };

    let journal = recover::journal_at(&paths.journal());
    let recovery = Recovery::of(&journal);

    println!("A previous setup was interrupted part-way through applying.");
    let written = recovery.written();
    if written.is_empty() {
        println!("It had not written anything yet.");
    } else {
        println!("It had written:");
        for change in written {
            println!("  · {}", describe(change));
        }
    }

    if !std::io::stdin().is_terminal() {
        eprintln!("\nerror: recovering an interrupted setup needs a terminal to choose.");
        eprintln!("Run `{PRODUCT} setup` interactively to resume, roll back, or start over.");
        return ExitCode::from(USAGE);
    }

    let env = paths.env_file();
    match recovery.resolve(ask_recovery_choice()) {
        Resolution::Resume => {
            println!("\nResuming.");
            resume_and_start(ctx, paths, progress).await
        }
        Resolution::RollBack(undos) => {
            if let Err(problem) = recover::undo(&undos, &env) {
                return complain(&problem);
            }
            println!("\nRolled back. Applying again.");
            resume_and_start(ctx, paths, progress).await
        }
        Resolution::StartOver(undos) => {
            if let Err(problem) = recover::undo(&undos, &env) {
                return complain(&problem);
            }
            discard(paths);
            println!("\nStarted over — nothing of the interrupted setup remains.");
            println!("Run `{PRODUCT} setup` to begin again.");
            ExitCode::SUCCESS
        }
    }
}

/// Re-apply the answers a stopped setup recorded, then bring the stack up.
async fn resume_and_start(mut ctx: Ctx, paths: &Paths, progress: Progress) -> ExitCode {
    let mut wizard = Wizard::resume(ctx.environment, progress);
    match setup::resume(&mut wizard, paths, ctx.stack, &stamp()) {
        Ok(()) => {
            ctx.settings = read_settings();
            println!("\nSetup is done — bringing your stack up.");
            start(&ctx).await
        }
        Err(problem) => complain(&problem),
    }
}

/// Which way out of an interrupted setup the operator chooses.
fn ask_recovery_choice() -> Choice {
    println!("\nWhat would you like to do?");
    println!("  1) Resume — finish applying from where it stopped");
    println!("  2) Roll back — undo what was written, then apply again");
    println!("  3) Start over — undo it and forget the answers");
    match read_line("Choose [1]:").as_str() {
        "2" => Choice::RollBack,
        "3" => Choice::StartOver,
        _ => Choice::Resume,
    }
}

/// A written change, said plainly enough for the operator to recognise.
fn describe(change: &Change) -> String {
    match &change.kind {
        Kind::Set { key, .. } => format!("the setting {key}"),
        Kind::Made { path } => format!("the directory {path}"),
        Kind::Created { resource, .. } => format!("a {resource}"),
    }
}

/// Remove what an interrupted setup left, so starting over leaves nothing behind.
fn discard(paths: &Paths) {
    let _ = std::fs::remove_file(paths.setup_progress());
    let _ = std::fs::remove_file(paths.journal());
}

/// Print a prompt and read the operator's trimmed answer.
fn read_line(prompt: &str) -> String {
    use std::io::Write as _;

    print!("{prompt} ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    line.trim().to_owned()
}

/// Check the environment before setup asks anything.
///
/// It runs the very check `lemonfiber doctor` runs for the environment — not a
/// second copy of it — so a missing container engine and one whose daemon is down
/// are told apart and remedied here in the same words as everywhere else. A broken
/// or undetermined result stops setup before a single question is asked; a healthy
/// one passes without a word.
async fn preflight(ctx: &Ctx) -> Result<(), ExitCode> {
    let report = match dispatch(
        Command::Doctor {
            only: Some(Category::Environment),
            disruptive: false,
        },
        ctx,
    )
    .await
    {
        Ok(Outcome::Doctor(report)) => report,
        // Asking for a diagnosis and being handed anything else cannot happen, but
        // is not worth a crash if it somehow does.
        Ok(_) => return Err(ExitCode::from(FAILURE)),
        Err(problem) => return Err(complain(&problem)),
    };

    if matches!(report.overall, Overall::Broken | Overall::Unknown) {
        render(&Outcome::Doctor(report), false);
        eprintln!("\nSetup needs these put right before it can go on.");
        return Err(ExitCode::from(PREFLIGHT));
    }
    Ok(())
}

/// The form setup brings up once the answers are applied.
///
/// The television form is the one the product is measured on — a fresh machine to
/// a working stack — and it is what a first run wants: an operator after only
/// movies or music switches with `up` once they are running.
const STARTER_FORM: &str = "tv";

/// Bring the stack up and report how it settled, the last step of a fresh setup.
async fn start(ctx: &Ctx) -> ExitCode {
    let up = Command::Up {
        forms: vec![STARTER_FORM.to_owned()],
    };
    match dispatch(up, ctx).await {
        Ok(outcome) => {
            render(&outcome, false);
            settled(&outcome)
        }
        Err(problem) => complain(&problem),
    }
}

/// The data location setup proposes when the operator does not name one.
///
/// A directory under this machine's data base, so a default run lands somewhere
/// real and writable; an operator with a NAS or a separate disk names that
/// instead.
fn default_data_location() -> PathBuf {
    here().map_or_else(
        || PathBuf::from("./media"),
        |paths| paths.data_dir().join("media"),
    )
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

/// Where this machine keeps lemonfiber's files.
///
/// Finding the platform's base directories is the surface's job: it means asking
/// the operating system, and there is nothing about it a test could catch that
/// running it would not. The layout beneath those bases is the core's, and is
/// tested there.
fn here() -> Option<Paths> {
    use etcetera::BaseStrategy as _;

    let strategy = etcetera::choose_base_strategy().ok()?;
    Some(Paths::rooted(&strategy.config_dir(), &strategy.data_dir()))
}

/// The operator's settings file, whether or not it exists yet.
///
/// Named even when absent, because `config set` has to be able to create it —
/// refusing to name a file until it exists would make setting the first setting
/// impossible.
fn configuration_file() -> Option<PathBuf> {
    here().map(|paths| paths.env_file())
}

/// Where an embedded stack is written so Compose can read it.
fn stack_directory() -> Option<PathBuf> {
    here().map(|paths| paths.stack())
}
