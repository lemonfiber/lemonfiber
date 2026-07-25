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
use lemonfiber_core::adapters::Local;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::Settings;
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
}

/// A general failure. Codes are meaningful so a script can branch on *why*
/// something failed rather than merely on whether it did.
const FAILURE: u8 = 1;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let Some(request) = cli.command else {
        println!("{PRODUCT} — run `lemonfiber --help` to see what it can do");
        return ExitCode::SUCCESS;
    };

    // The path outlives the process, and `Source` is Copy so it can be handed
    // around freely; leaking one allocation at startup buys both.
    let stack = match cli.stack_dir {
        Some(path) => Source::External(Box::leak(path.into_boxed_path())),
        None => Source::Embedded(&STACK),
    };

    let settings = Settings {
        env_file: environment_file(),
        ..Settings::default()
    };

    let mut ctx = Ctx::new(Arc::new(Local), stack, settings);
    if cli.dry_run {
        ctx = ctx.rehearsing();
    }

    let command = match request {
        Request::Version => Command::Version,
    };

    match dispatch(command, &ctx).await {
        Ok(outcome) => {
            render(&outcome, cli.json);
            ExitCode::SUCCESS
        }
        Err(problem) => {
            eprintln!("{}: {}", problem.code, problem.summary);
            eprintln!("\n  {}\n", problem.meaning);
            for remedy in &problem.remedies {
                eprintln!("  → {}", remedy.action);
            }
            ExitCode::from(FAILURE)
        }
    }
}

/// The operator's environment file, when they have one.
///
/// Finding the platform's base directories is the surface's job: it means asking
/// the operating system, and there is nothing about it a test could catch that
/// running it would not. The layout beneath those bases is the core's, and is
/// tested there.
fn environment_file() -> Option<PathBuf> {
    use etcetera::BaseStrategy as _;

    let strategy = etcetera::choose_base_strategy().ok()?;
    let paths = Paths::rooted(&strategy.config_dir(), &strategy.data_dir());
    let env = paths.env_file();
    env.is_file().then_some(env)
}

/// Render an outcome, for a person or for a script.
fn render(outcome: &Outcome, json: bool) {
    if json {
        match serde_json::to_string(&outcome.clone().envelope()) {
            Ok(text) => println!("{text}"),
            Err(err) => eprintln!("could not render output: {err}"),
        }
        return;
    }

    match outcome {
        Outcome::Version(report) => {
            println!("{PRODUCT} {}", report.binary);
            println!("stack {}", report.stack);
            println!("manifest schema {:?}", report.supported_schema);
            match &report.compose {
                Some(version) => println!("compose {version}"),
                None => println!("compose not reachable"),
            }
        }
    }
}
