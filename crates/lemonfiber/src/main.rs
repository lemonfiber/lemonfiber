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
use lemonfiber_core::app::{
    dispatch, logs, pull_progress, supervise, Command, Ctx, Outcome, QualityAction, WATCH,
};
use lemonfiber_core::audio::Format;
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::{
    data_root_from_env, indexer_from_env, ip_echo_from_env, port_forward_from_env,
    service_user_from_env, store, Protocols, Settings,
};
use lemonfiber_core::doctor::{Category, Overall};
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::{Disposition, Envelope, Triggered, UpgradeReport};
use lemonfiber_core::platform::{Environment, HOST_OS};
use lemonfiber_core::ports::docker::LogQuery;
use lemonfiber_core::ports::process::Progress as PullEvent;
use lemonfiber_core::quality::Preset;
use lemonfiber_core::recyclarr::Kind;
use lemonfiber_core::stack::Source;

mod archive;
mod maintain;
mod nntp;
mod prompt;
mod render;
mod setup;
use prompt::SetupFlags;
use render::{render, watched};
use setup::{greet, run_setup};

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
    ///
    /// Interactive by default. Given the flags below, it runs unattended: each
    /// answers a question the wizard would otherwise ask, and `--yes` stands in for
    /// the confirmation. A non-interactive run missing a flag it needs is told
    /// which, rather than left waiting on input that will not come.
    Setup {
        /// Apply without a prompt to confirm — required for an unattended run.
        #[arg(long)]
        yes: bool,
        /// How to fetch content: `both`, `usenet`, `torrent`, or `none`.
        #[arg(long, value_name = "PROTOCOLS")]
        protocols: Option<String>,
        /// Where the library and downloads live.
        #[arg(long, value_name = "PATH")]
        data_location: Option<PathBuf>,
        /// An indexer's API base URL.
        #[arg(long, value_name = "URL")]
        indexer_url: Option<String>,
        /// The indexer's API key.
        #[arg(long, value_name = "KEY")]
        indexer_key: Option<String>,
        /// The Usenet provider's hostname.
        #[arg(long, value_name = "HOST")]
        usenet_host: Option<String>,
        /// The port the Usenet provider answers on (defaults to 563).
        #[arg(long, value_name = "PORT")]
        usenet_port: Option<u16>,
        /// The Usenet account username.
        #[arg(long, value_name = "USER")]
        usenet_user: Option<String>,
        /// The Usenet account password.
        #[arg(long, value_name = "PASS")]
        usenet_pass: Option<String>,
        /// Whether the Usenet connection uses TLS (defaults to yes).
        #[arg(long, value_name = "BOOL")]
        usenet_tls: Option<bool>,
        /// How to serve the library: `docker`, `native`, or `none`.
        #[arg(long, value_name = "MODE")]
        library: Option<String>,
        /// The container user, as `UID:GID`.
        #[arg(long, value_name = "UID:GID")]
        service_user: Option<String>,
        /// Whether others in the home will use it.
        #[arg(long, value_name = "BOOL")]
        household: Option<bool>,
        /// Whether to start the stack when the machine boots.
        #[arg(long, value_name = "BOOL")]
        autostart: Option<bool>,
    },
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
    /// Choose how good your media should look, in plain language.
    Quality {
        #[command(subcommand)]
        action: QualityCommand,
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
    /// Adopt your current edits as lemonfiber's expected state.
    ///
    /// A value you changed by hand reports as drift until you adopt it; once
    /// adopted it is kept across future seeds and restores. Wires what is missing
    /// as a seed does, and promotes every drifted value to yours.
    Adopt,
    /// Back up your configuration to an archive, so it stops being precious.
    Backup {
        /// Back up one service's configuration instead of the whole stack.
        #[arg(long, value_name = "SERVICE")]
        service: Option<String>,
    },
    /// Restore your configuration from a backup archive.
    ///
    /// Verifies the archive and lists what it holds before anything is
    /// overwritten. A restore onto a different data root is refused until
    /// `--repoint` accepts moving it to this machine's.
    Restore {
        /// The archive to restore from.
        archive: PathBuf,
        /// Accept re-pointing to this machine's data root where it differs.
        #[arg(long)]
        repoint: bool,
    },
}

/// What to do with settings.
#[derive(Debug, Subcommand)]
enum QualityCommand {
    /// Show the quality choice in force, and what each preset means and costs.
    Show,
    /// Choose a preset — for everything, or for one media type.
    Set {
        /// The preset: space-saving, balanced, high-quality, or maximum.
        preset: String,
        /// Apply it to one media type (tv or movies) rather than everything.
        #[arg(long = "for", value_name = "MEDIA_TYPE")]
        media_type: Option<String>,
        /// Confirm a choice this machine would have to transcode in software.
        #[arg(long)]
        confirm: bool,
    },
    /// Re-assert the recorded preset over a Recyclarr config you have hand-edited.
    ///
    /// An ordinary run keeps your edits; this is the explicit consent to let the
    /// preset win instead.
    Reapply,
    /// Upgrade existing content to the chosen quality — re-download what is already
    /// here at the higher quality.
    ///
    /// A large, bandwidth-expensive operation, separate from a preset change (which
    /// only affects future acquisitions). States the cost and does nothing until
    /// `--confirm`.
    Upgrade {
        /// Go ahead and trigger the re-search, having seen the cost.
        #[arg(long)]
        confirm: bool,
    },
}

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
        Request::Setup {
            yes,
            protocols,
            data_location,
            indexer_url,
            indexer_key,
            usenet_host,
            usenet_port,
            usenet_user,
            usenet_pass,
            usenet_tls,
            library,
            service_user,
            household,
            autostart,
        } => {
            let raw = prompt::RawSetup {
                yes,
                protocols,
                data_location,
                indexer_url,
                indexer_key,
                usenet_host,
                usenet_port,
                usenet_user,
                usenet_pass,
                usenet_tls,
                library,
                service_user,
                household,
                autostart,
            };
            let flags = match SetupFlags::parse(raw) {
                Ok(flags) => flags,
                Err(message) => {
                    eprintln!("error: {message}");
                    return ExitCode::from(USAGE);
                }
            };
            return run_setup(ctx, flags).await;
        }
        Request::Version => Command::Version,
        Request::Up { forms } => Command::Up { forms },
        Request::Down { forms } => Command::Down { forms },
        Request::Restart { form, services } => Command::Restart {
            forms: vec![form],
            services,
        },
        // A pull is watched as it happens rather than waited on in silence, so like
        // streaming and watching it runs its own way instead of through dispatch.
        Request::Pull { forms } => return pull(&ctx, &forms, cli.json).await,
        Request::Ps { forms } => Command::Ps { forms },
        Request::Config { action } => configuration(action),
        Request::Quality { action } => match quality(action) {
            Ok(command) => command,
            Err(code) => return ExitCode::from(code),
        },
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
        Request::Adopt => Command::Adopt,
        // Backup and restore drive their own executors over the tar adapter and
        // render their own reports, like setup — they are not one value from
        // dispatch. They take the context by value for the settings they read.
        Request::Backup { service } => return maintain::run_backup(ctx, service, cli.json).await,
        Request::Restore { archive, repoint } => {
            return maintain::run_restore(ctx, archive, repoint, cli.json).await
        }
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
        // non-zero result — but the two reasons differ. A refused conflict (two
        // \*arrs on one root folder) is something the operator wrote that lemonfiber
        // will not act on until they resolve it, so it earns VALIDATION; work merely
        // left skipped or failed may complete on a re-run, so it stays FAILURE. A
        // script can then tell "fix your config" from "wait and retry".
        Outcome::Seed(report) => {
            if report.is_complete() {
                ExitCode::SUCCESS
            } else if report.blocked().is_empty() {
                ExitCode::from(FAILURE)
            } else {
                ExitCode::from(VALIDATION)
            }
        }
        // A held quality choice was not recorded — it needs the operator to
        // confirm a preset this machine would software-transcode, so a script sees
        // a non-zero result it can act on rather than a false success.
        Outcome::Quality(report) => match report.disposition {
            Disposition::Held => ExitCode::from(VALIDATION),
            Disposition::Shown
            | Disposition::Recorded
            | Disposition::Rehearsed
            | Disposition::Reapplied
            | Disposition::WouldReapply => ExitCode::SUCCESS,
        },
        Outcome::Upgrade(report) => upgrade_exit(report),
        // The music choice is recorded even when the service could not be reached, so
        // only a service that refused the change is a failure; a rehearsal or a service
        // still coming up has still recorded the choice.
        Outcome::Music(report) => {
            if matches!(report.outcome, Some(Triggered::Failed { .. })) {
                ExitCode::from(FAILURE)
            } else {
                ExitCode::SUCCESS
            }
        }
        Outcome::Version(_) | Outcome::Lifecycle(_) | Outcome::Config(_) | Outcome::Status(_) => {
            ExitCode::SUCCESS
        }
    }
}

/// The exit code an upgrade earns.
///
/// An unconfirmed upgrade stated its cost and did nothing, so a script sees a non-zero
/// result telling it to confirm. A service that refused is a failure; a run where
/// nothing was actually started — every service still coming up, or none present — is a
/// failure too, so success means at least one re-search began and none was refused.
fn upgrade_exit(report: &UpgradeReport) -> ExitCode {
    let outcome = |want: fn(&Triggered) -> bool| {
        report
            .media
            .iter()
            .filter_map(|media| media.outcome.as_ref())
            .any(want)
    };
    if !report.confirmed {
        ExitCode::from(VALIDATION)
    } else if outcome(|state| matches!(state, Triggered::Failed { .. })) {
        ExitCode::from(FAILURE)
    } else if outcome(|state| matches!(state, Triggered::Started)) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(FAILURE)
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
        indexer: indexer_from_env(&recorded),
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

/// A quality subcommand, or the exit code for input that cannot be understood.
///
/// The preset and the media type are named in plain words the operator types, so a
/// name that is neither is a mistake to correct rather than a request to run — it
/// is refused here, before the core is reached, with the valid names spelled out.
fn quality(action: QualityCommand) -> Result<Command, u8> {
    let action = match action {
        QualityCommand::Show => QualityAction::Show,
        QualityCommand::Set {
            preset,
            media_type,
            confirm,
        } => {
            // Music has no resolution: `--for music` chooses an audio format instead of a
            // resolution preset, and reaches the service rather than only recording, so it
            // routes to its own command.
            if media_type.as_deref() == Some("music") {
                let Some(format) = Format::from_label(&preset) else {
                    eprintln!(
                        "error: no music format named `{preset}` \
                         (try compact, lossless, or hi-res)"
                    );
                    return Err(USAGE);
                };
                return Ok(Command::QualityMusic { format });
            }
            let Some(preset) = Preset::from_label(&preset) else {
                eprintln!(
                    "error: no quality preset named `{preset}` \
                     (try space-saving, balanced, high-quality, or maximum)"
                );
                return Err(USAGE);
            };
            if let Some(media_type) = &media_type {
                if !Kind::ALL.iter().any(|kind| kind.media_type() == media_type) {
                    eprintln!(
                        "error: no media type named `{media_type}` (try tv, movies, or music)"
                    );
                    return Err(USAGE);
                }
            }
            QualityAction::Set {
                preset,
                media_type,
                confirm,
            }
        }
        QualityCommand::Reapply => QualityAction::Reapply,
        // Upgrade is its own command, not a quality action, since it reaches the
        // services rather than only reading or writing the recorded choice.
        QualityCommand::Upgrade { confirm } => return Ok(Command::QualityUpgrade { confirm }),
    };
    Ok(Command::Quality(action))
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

/// Pull the images the forms need, showing each as it downloads.
///
/// Image pulls are gigabytes and take minutes; a silent wait reads as a hang, so
/// the operator is told the wait is coming and then shown Compose's per-image
/// progress as it arrives.
async fn pull(ctx: &Ctx, forms: &[String], json: bool) -> ExitCode {
    match pull_showing(ctx, forms, json).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

/// Stream a pull to the screen, or report why it could not run.
///
/// Split from [`pull`] so setup's Start phase can pull-with-progress and, only if
/// that succeeds, go on to bring the stack up — a `Result` it can branch on rather
/// than an exit code it cannot compare.
async fn pull_showing(ctx: &Ctx, forms: &[String], json: bool) -> Result<(), ExitCode> {
    // A rehearsal reports what would be pulled rather than pulling it; the buffered
    // path already renders that, so a dry run borrows it rather than streaming.
    if ctx.dry_run {
        return pull_rehearsal(ctx, forms, json).await;
    }

    // The expected-duration statement, before the wait — qualitative, because the
    // real figure depends on the operator's line and what is already cached.
    if !json {
        println!("Pulling images — usually a few minutes, and several gigabytes.");
    }

    let mut progress = pull_progress(ctx, forms)
        .await
        .map_err(|problem| complain(&problem))?;
    let mut failed = false;
    while let Some(event) = progress.recv().await {
        match event {
            PullEvent::Line(line) => emit_pull_line(&line, json),
            // A non-zero exit is the pull's own report that an image did not come
            // down; the operator is told, and a script sees a non-zero code.
            PullEvent::Ended(status) => failed = status != Some(0),
        }
    }

    if failed {
        if !json {
            eprintln!("\nSome images could not be pulled — check the output above.");
        }
        return Err(ExitCode::from(FAILURE));
    }
    Ok(())
}

/// A dry run's rehearsal: report what a pull would fetch without fetching it,
/// borrowing the buffered path's rendering rather than streaming a pull that will
/// not happen.
async fn pull_rehearsal(ctx: &Ctx, forms: &[String], json: bool) -> Result<(), ExitCode> {
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

/// Render one line of a pull's progress — wrapped in an envelope under `--json`,
/// indented beneath the pull for a person to read.
fn emit_pull_line(line: &str, json: bool) {
    if !json {
        println!("  {line}");
        return;
    }
    match Envelope::new("pull", line).to_json() {
        Some(text) => println!("{text}"),
        None => eprintln!("this line could not be rendered as JSON"),
    }
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

/// Print a prompt and read the operator's trimmed answer.
///
/// The setup surface reads lines through this; it lives here because a prompt is
/// the plainest surface primitive there is and the module that reads and renders
/// is the binary's, not the setup flow's alone.
pub(crate) fn read_line(prompt: &str) -> String {
    use std::io::Write as _;

    print!("{prompt} ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    line.trim().to_owned()
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
