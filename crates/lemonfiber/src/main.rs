//! The `lemonfiber` binary — the surfaces, over one core.
//!
//! This crate is the only one that renders. It turns input into a command,
//! hands it to the core, and renders what comes back; it makes no decisions of
//! its own, which is why the same request behaves identically whether it arrived
//! as a subcommand, a keypress or an HTTP route.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use include_dir::{include_dir, Dir};
use lemonfiber_core::adapters::{Daemon, Disk, Local, System};
use lemonfiber_core::app::{dispatch, logs, supervise, Command, Ctx, Outcome, WATCH};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::{
    data_root_from_env, ip_echo_from_env, port_forward_from_env, service_user_from_env, store,
    Protocols, Settings,
};
use lemonfiber_core::docker::{Condition, Service, State};
use lemonfiber_core::doctor::{Category, Overall, Verdict};
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::{
    ConfigReport, DoctorReport, Envelope, LifecycleReport, StatusReport, SupervisionReport,
    VersionReport,
};
use lemonfiber_core::platform::{Environment, HOST_OS};
use lemonfiber_core::ports::docker::LogQuery;
use lemonfiber_core::seed::{Report as SeedReport, State as SeedState};
use lemonfiber_core::stack::Source;
use lemonfiber_core::PRODUCT;

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
        println!("{PRODUCT} — run `lemonfiber --help` to see what it can do");
        return ExitCode::SUCCESS;
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

    // Settings come from the operator's own file, so what runs reflects what
    // they configured rather than what a default happened to be.
    let env_file = configuration_file();
    let recorded = env_file
        .as_deref()
        .and_then(|path| store::read(path).ok())
        .unwrap_or_default();

    let settings = Settings {
        protocols: Protocols::from_env(&recorded),
        ip_echo: ip_echo_from_env(&recorded),
        data_root: data_root_from_env(&recorded),
        storage_state: here().map(|paths| paths.storage_state()),
        service_user: service_user_from_env(&recorded),
        port_forward: port_forward_from_env(&recorded),
        env_file,
        stack_dir: stack_directory(),
        ..Settings::default()
    };

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

/// What a watch did once its location was lost.
fn watched(report: &SupervisionReport, json: bool) {
    if json {
        match Envelope::new("watch", report.clone()).to_json() {
            Some(text) => println!("{text}"),
            None => eprintln!("this report could not be rendered as JSON"),
        }
        return;
    }

    println!("the watch ended: {}", report.reason);
    if report.stopped {
        println!("stopped: {}", report.forms.join(", "));
    } else {
        println!(
            "could not stop {} — check the services by hand",
            report.forms.join(", ")
        );
    }
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

/// Render an outcome, for a person or for a script.
///
/// One renderer per answer, rather than one function that knows all four. They
/// have nothing in common beyond arriving here: what a version report owes an
/// operator and what a lifecycle report owes them are different questions, and
/// a single body deciding both reads as one thing with four moods.
fn render(outcome: &Outcome, json: bool) {
    if json {
        machine_readable(outcome);
        return;
    }

    match outcome {
        Outcome::Version(report) => versions(report),
        Outcome::Config(report) => settings(report),
        Outcome::Lifecycle(report) => lifecycle(report),
        Outcome::Status(report) => status(report),
        Outcome::Doctor(report) => diagnosis(report),
        Outcome::Seed(report) => seeding(report),
    }
}

/// What seeding wired, connection by connection, with what a re-run still owes
/// named last so it is the thing the operator is left looking at.
fn seeding(report: &SeedReport) {
    for wiring in &report.wirings {
        match &wiring.state {
            SeedState::Wired => println!("  ✓ {}   wired", wiring.connection),
            SeedState::AlreadyWired => println!("  ✓ {}   already wired", wiring.connection),
            SeedState::Drifted => println!("  · {}   left as you set it", wiring.connection),
            SeedState::Skipped { reason } => {
                println!("  ? {}   skipped", wiring.connection);
                println!("      {reason}");
            }
            SeedState::Failed { detail } => {
                println!("  ✗ {}   {detail}", wiring.connection);
            }
        }
    }
    let outstanding = report.outstanding();
    if outstanding.is_empty() {
        println!("\nEverything is wired.");
    } else {
        println!(
            "\n{} left to wire — run seed again once ready.",
            outstanding.len()
        );
    }
}

/// What the diagnostic checks found, finding by finding.
///
/// Each finding leads with a mark that reads at a glance and the plain evidence
/// behind it; a non-passing one carries the reason and what to do, because a
/// finding without a remedy is a fault report rather than a diagnosis.
fn diagnosis(report: &DoctorReport) {
    for finding in &report.findings {
        match &finding.verdict {
            Verdict::Pass { note } => match note {
                Some(note) => println!("  ✓ {}   {note}", finding.title),
                None => println!("  ✓ {}", finding.title),
            },
            Verdict::Warn(problem) => {
                println!("  ! {}   {}", finding.title, problem.summary);
                remedies(problem);
            }
            Verdict::Fail(problem) => {
                println!("  ✗ {}   {}", finding.title, problem.summary);
                remedies(problem);
            }
            Verdict::Unverified { reason, remedy } => {
                println!("  ? {}   UNVERIFIED", finding.title);
                println!("      {reason}");
                println!("      → {}", remedy.action);
                if let Some(detail) = &remedy.detail {
                    println!("        {detail}");
                }
            }
            Verdict::Skipped { reason } => {
                println!("  – {}   skipped: {reason}", finding.title);
            }
        }
    }

    println!("\n{}", overall(report.overall));
}

/// The problem's meaning and remedies, indented under a finding.
fn remedies(problem: &Problem) {
    println!("      {}", problem.meaning);
    for remedy in &problem.remedies {
        println!("      → {}", remedy.action);
        if let Some(detail) = &remedy.detail {
            println!("        {detail}");
        }
    }
}

/// The one-line verdict a diagnosis amounts to.
fn overall(overall: Overall) -> &'static str {
    match overall {
        Overall::Healthy => "healthy — everything checked passed",
        Overall::Degraded => "degraded — working, with warnings",
        Overall::Broken => "broken — something needs attention",
        Overall::Unknown => "unknown — health could not be established",
    }
}

/// The same answer, for something that will parse it.
fn machine_readable(outcome: &Outcome) {
    match outcome.clone().envelope().to_json() {
        Some(text) => println!("{text}"),
        None => eprintln!("this outcome could not be rendered as JSON"),
    }
}

/// What versions are in play.
fn versions(report: &VersionReport) {
    println!("{PRODUCT} {}", report.binary);
    println!("stack {}", report.stack);
    println!("manifest schema {:?}", report.supported_schema);
    match &report.compose {
        Some(version) => println!("compose {version}"),
        None => println!("compose not reachable"),
    }
}

/// What the operator has configured.
fn settings(report: &ConfigReport) {
    for setting in &report.settings {
        println!("{}={}", setting.key, setting.value);
    }
    if report.changed {
        // A rehearsal reports what it would do, so it must not claim it saved.
        println!(
            "{}",
            if report.rehearsed {
                "would save"
            } else {
                "saved"
            }
        );
    }
}

/// What a lifecycle command did, or would have done.
fn lifecycle(report: &LifecycleReport) {
    if report.rehearsed {
        println!("would run:\n  {}", report.command.join(" "));
    }
    println!("{}: {}", report.action, report.profiles.join(", "));

    // Saying what was left out, and that it was deliberate, before the operator
    // goes looking for a service that was never going to start.
    if !report.dropped.is_empty() {
        println!(
            "left out (no provider configured): {}",
            report.dropped.join(", ")
        );
    }

    if let Some(condition) = report.condition {
        println!("\n{}", describe(condition));
        show(&report.services);
    }
}

/// What each service is doing.
fn status(report: &StatusReport) {
    println!("{}", describe(report.condition));
    show(&report.services);
}

/// A condition, as a sentence rather than as a word.
fn describe(condition: Condition) -> &'static str {
    match condition {
        Condition::Inactive => "nothing is running",
        Condition::Degraded => "running, and something needs attention",
        Condition::Partial => "partly up",
        Condition::Active => "everything is up",
    }
}

/// What each service is doing, one per line.
fn show(services: &[Service]) {
    for service in services {
        let state = match service.state {
            State::Absent => "absent".to_owned(),
            State::Stopped => "stopped".to_owned(),
            State::Starting => "starting".to_owned(),
            State::Running => "running".to_owned(),
            State::Healthy => "healthy".to_owned(),
            State::Unhealthy => "unhealthy".to_owned(),
            State::CrashLooping => "crash-looping".to_owned(),
            State::HostManaged => "host-managed".to_owned(),
            // The code is the whole reason this is not simply "stopped", so it
            // is shown rather than left for the operator to go and find.
            State::Failed => match service.exit {
                Some(code) => format!("failed ({code})"),
                None => "failed".to_owned(),
            },
        };
        println!("  {:<14} {:<14} {}", service.id, state, service.name);
    }
}
