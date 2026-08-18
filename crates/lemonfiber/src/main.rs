//! The `lemonfiber` binary — the surfaces, over one core.
//!
//! This crate is the only one that renders. It turns input into a command,
//! hands it to the core, and renders what comes back; it makes no decisions of
//! its own, which is why the same request behaves identically whether it arrived
//! as a subcommand, a keypress or an HTTP route.

use std::process::ExitCode;

use clap::Parser;
use lemonfiber_core::app::{dispatch, Command, Ctx};
use lemonfiber_core::doctor::Category;

mod archive;
mod cli;
mod context;
mod dashboard;
mod engine;
mod exit;
mod keyboard;
mod maintain;
mod prompt;
mod render;
mod repair;
mod setup;
mod support;
mod terminal;
mod translate;
mod walkthrough;

use cli::{Cli, Request};
use context::{context, here};
use engine::{guard, pull, stream};
use exit::{complain, no_config_home, settled, USAGE};
use keyboard::{Console, Keyboard};
use prompt::SetupFlags;
use render::render;
use setup::{greeting, setting_up};
use translate::{configuration, quality};
use walkthrough::walk;

#[tokio::main]
async fn main() -> ExitCode {
    let mut cli = Cli::parse();

    let Some(request) = cli.command else {
        let ctx = context(cli.stack_dir.take(), cli.dry_run);
        let Some(paths) = here() else {
            // With nowhere to keep its files there is nothing to offer and nothing
            // to point at, so the plain pointer is the only honest thing left.
            println!("lemonfiber — run `lemonfiber --help` to see what it can do");
            return ExitCode::SUCCESS;
        };
        return greeting(ctx, &paths, &Console).await;
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
        // Setup is a conversation and then a stack coming up, not a value that
        // arrives once, so like streaming and watching it runs its own way. It
        // takes the context by value because it rewrites the settings mid-run.
        Request::Setup { flags } => return setup_from(ctx, flags).await,
        Request::Version => Command::Version,
        Request::Forms => Command::Forms,
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
        Request::Doctor {
            only,
            disruptive,
            accept,
            mending,
        } => {
            // Repairing is its own errand: it looks, offers, acts and looks again, and
            // renders what became of each — not one value from dispatch. A plain run falls
            // through to the diagnosis below and changes nothing.
            if mending.fix {
                return repair::run(ctx, mending, &Keyboard, cli.json).await;
            }
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
            Command::Doctor {
                only,
                disruptive,
                accept,
            }
        }
        // The term is taken as words so it can be typed unquoted; joined back into the
        // title as said.
        Request::Trace { term, season } => Command::Trace {
            term: term.join(" "),
            season,
        },
        // A walkthrough narrates for minutes and produces one report at its end, not a
        // value that arrives once, so like streaming and watching it runs its own way.
        Request::Walkthrough { item } => {
            return walk(&ctx, &item.join(" "), cli.json).await;
        }
        Request::Household { member } => Command::Household { member },
        Request::Stuck => Command::Stuck,
        Request::Seed => Command::Seed,
        Request::Adopt => Command::Adopt,
        Request::Reset { confirm } => Command::Reset { confirm },
        // Backup and restore drive their own executors over the tar adapter and
        // render their own reports, like setup — they are not one value from
        // dispatch. They take the context by value for the settings they read.
        Request::Backup { service } => {
            let Some(paths) = here() else {
                return no_config_home();
            };
            return maintain::run_backup(ctx, paths, service, cli.json).await;
        }
        // A bundle drives its own executor over the same tar adapter, and renders both
        // of the answers it can give — what one would hold, and where one went.
        Request::Support(asked) => return support::run(ctx, asked, cli.json).await,
        Request::Restore { archive, repoint } => {
            let Some(paths) = here() else {
                return no_config_home();
            };
            return maintain::run_restore(ctx, paths, archive, repoint, cli.json).await;
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

/// A quality subcommand, or the exit code for input that cannot be understood.
///
/// The preset and the media type are named in plain words the operator types, so a
/// name that is neither is a mistake to correct rather than a request to run — it
/// is refused here, before the core is reached, with the valid names spelled out.
/// Run setup from what the command line carried, or refuse it with a usage code.
///
/// The flags are validated before anything is applied, so a contradictory pair is a
/// mistake to name rather than a half-configured stack — the same shape as `quality`
/// below, which turns its own subcommand into a value or a code to exit with.
async fn setup_from(ctx: Ctx, raw: prompt::RawSetup) -> ExitCode {
    let flags = match SetupFlags::parse(raw) {
        Ok(flags) => flags,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::from(USAGE);
        }
    };
    let Some(paths) = here() else {
        return no_config_home();
    };
    setting_up(ctx, &paths, &Console, flags).await
}
