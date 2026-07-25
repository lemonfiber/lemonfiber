//! The `lemonfiber` binary — the surfaces, over one core.
//!
//! This crate is the only one that renders. It turns input into a command,
//! hands it to the core, and renders what comes back; it makes no decisions of
//! its own, which is why the same request behaves identically whether it arrived
//! as a subcommand, a keypress or an HTTP route.

use std::process::ExitCode;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use lemonfiber_core::adapters::Local;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::PRODUCT;

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

    let mut ctx = Ctx::new(Arc::new(Local));
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
            println!("manifest schema {:?}", report.supported_schema);
            match &report.compose {
                Some(version) => println!("compose {version}"),
                None => println!("compose not reachable"),
            }
        }
    }
}
