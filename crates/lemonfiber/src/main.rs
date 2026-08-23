//! The `lemonfiber` binary — the surfaces, over one core.
//!
//! This crate is the only one that renders. It turns input into a command,
//! hands it to the core, and renders what comes back; it makes no decisions of
//! its own, which is why the same request behaves identically whether it arrived
//! as a subcommand, a keypress or an HTTP route.

use std::process::ExitCode;

use clap::Parser;
use lemonfiber::cli::{Cli, RawSetup, RawUi, Request};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::doctor::Category;

mod archive;
mod context;
mod dashboard;
mod engine;
mod exit;
mod keyboard;
mod logs;
mod maintain;
mod pane;
mod prompt;
mod render;
mod repair;
mod say;
mod setup;
mod stopping;
mod support;
mod terminal;
mod translate;
mod ui;
mod walkthrough;

use crate::say::{complain, say};
use context::{context, here};
use engine::{guard, pull, settle, start, stream};
use exit::{complain, no_config_home, settled, USAGE};
use keyboard::{Console, Keyboard};
use prompt::SetupFlags;
use render::render;
use render::stack::Doing;
use setup::{greeting, setting_up};
use translate::{configuration, quality};
use walkthrough::walk;

/// Logs as a screen, or logs as a stream.
///
/// Different answers to the same request, and only one of them can have the
/// terminal — so which it is has to be settled before either starts.
async fn read_logs(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    follow: bool,
    watch: bool,
    tail: u32,
    json: bool,
) -> ExitCode {
    if watch {
        terminal::watching(ctx, forms, services, tail).await
    } else {
        stream(ctx, forms, services, follow, tail, json).await
    }
}

/// What the environment says about this terminal's character set.
///
/// Read here rather than deeper in, because this is the edge: what a locale means
/// is decided in [`crate::say`] where a test can reach it, and only this knows
/// where the locale came from. The same division the log viewer makes over
/// `NO_COLOR`.
///
/// Read in the order POSIX reads it: a specific override, then the character
/// category, then the general setting.
fn locale() -> Option<String> {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok().filter(|said| !said.is_empty()))
}

#[tokio::main]
async fn main() -> ExitCode {
    // Settled before anything is printed, because it decides how everything is.
    say::settle(locale().as_deref());
    let mut cli = Cli::parse();
    // And who it is for, which decides the same for every failure without any of
    // the twenty-six places that report one having to be told.
    say::settle_audience(cli.json);

    let Some(request) = cli.command else {
        let ctx = context(cli.stack_dir.take(), cli.dry_run, cli.force);
        let Some(paths) = here() else {
            // With nowhere to keep its files there is nothing to offer and nothing
            // to point at, so the plain pointer is the only honest thing left.
            say!("lemonfiber — run `lemonfiber --help` to see what it can do");
            return ExitCode::SUCCESS;
        };
        return greeting(ctx, &paths, &Console).await;
    };

    let ctx = context(cli.stack_dir.take(), cli.dry_run, cli.force);

    let command = match request {
        // Streaming is not a value that arrives once, so it does not become a
        // command and does not go through dispatch. It still goes through the
        // core, which is the part that matters.
        Request::Logs {
            services,
            form,
            follow,
            watch,
            tail,
        } => return read_logs(&ctx, &form, &services, follow, watch, tail, cli.json).await,
        // A watch is long-running and produces one report at its end, not a value
        // that arrives once, so like streaming it does not go through dispatch.
        Request::Watch { forms } => return guard(&ctx, &forms, cli.json).await,
        // Setup is a conversation and then a stack coming up, not a value that
        // arrives once, so like streaming and watching it runs its own way. It
        // takes the context by value because it rewrites the settings mid-run.
        Request::Setup { flags } => return setup_from(ctx, flags).await,
        Request::Version => Command::Version,
        // Naming nothing asks what forms there are; naming one asks what it would
        // come to. Two questions about the same subject, so one word answers both.
        Request::Forms { forms } if forms.is_empty() => Command::Forms,
        Request::Forms { forms } => Command::Preview { forms },
        // Starting and stopping both say what they will affect first. Restarting does
        // not: it affects what is running rather than what a form holds — a restart of
        // one named service touches one service — so "starts eight services" before it
        // would be a sentence about the wrong set.
        Request::Up { forms, services } => {
            return starting(&ctx, &forms, &services, cli.json).await
        }
        Request::Down {
            forms,
            services,
            wait,
            yes,
        } => halting(&ctx, forms, services, wait, yes, cli.json).await,
        // Not announced beforehand the way starting is. A switch's own report is the
        // announcement — what stopped, what started, and what was left alone — and
        // saying "starts eight services" first would name the wrong set twice over.
        Request::Switch { forms } => Command::Switch { forms },
        Request::Restart { form, services } => restarting(form, services),
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
            if mending.acts() {
                let Some(paths) = here() else {
                    return no_config_home();
                };
                return repair::run(ctx, paths, mending, &Keyboard, cli.json).await;
            }
            let only = match narrowed(only.as_deref()) {
                Ok(only) => only,
                Err(code) => return code,
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
        Request::Walkthrough { item } => return walk(&ctx, &item.join(" "), cli.json).await,
        // Answered from a table compiled into the binary, so unlike every command
        // below it this one needs neither a stack nor a daemon to say anything.
        Request::Explain { word } => return explaining(&word.join(" "), cli.dry_run),
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
        // The web surface holds the process until it is stopped, and answers many
        // requests rather than producing one value, so like the dashboard and the
        // log viewer it runs its own way instead of through dispatch. It takes the
        // context by value because every request it answers reaches through it.
        Request::Ui(asked) => return serving(ctx, asked).await,
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

/// The app compiled into this binary, and there is not one yet.
///
/// The app arrives as a pinned submodule at `assets/web`, embedded exactly as the
/// stack beside it is. That submodule does not exist, so this build carries no app
/// and says so when a browser asks for one. What reads an embedded app is built
/// and proven; what it would read is what is missing. See
/// `.docs/architecture/embedded-stack.md` for the shape it arrives in.
const EMBEDDED_APP: Option<lemonfiber_core::frontend::Source> = None;

/// Serve the web interface until the operator stops the process.
///
/// What ends the loop is a dependency rather than a choice made inside it, so the
/// surface can be started, asked something, and stopped, in a test. A real run is
/// handed a signal that never arrives.
async fn serving(ctx: Ctx, asked: RawUi) -> ExitCode {
    let asked = ui::Asked {
        port: asked.port,
        browser: !asked.no_browser,
        assets: asked.assets,
    };
    ui::run(ctx, asked, EMBEDDED_APP, Box::pin(std::future::pending())).await
}

/// Say what a word means, or say that this product does not explain that one.
///
/// A word with no entry is a refusal rather than an empty answer: somebody who typed
/// `indexr` needs to be told they did, and a blank response reads as "it means
/// nothing" — a wrong answer where they wanted an absent one. Reported through the
/// same error model as everything else, so it carries a code and a way forward
/// rather than being this one command's private way of saying no.
fn explaining(word: &str, rehearsing: bool) -> ExitCode {
    let Some(lines) = render::glossary::explained(word, say::for_a_parser()) else {
        return complain(&render::glossary::unrecognised(word));
    };
    lines.print();
    // A script fetching the text is not a person learning the word. Acknowledgement
    // is about what this operator has been told, and recording a machine's lookup
    // would quietly stop explaining it to the person who never made one.
    // A rehearsal changes nothing, which this record is not exempt from. And a
    // script fetching the text is not a person learning the word: recording a
    // machine's lookup would quietly stop explaining it to the operator who never
    // made one.
    if !say::for_a_parser() {
        context::remember(&[word], rehearsing);
    }
    ExitCode::SUCCESS
}

/// Say what starting these forms will start, before it starts.
///
/// The plan comes from the core, resolved exactly as the command about to run
/// will resolve it, so this is the same answer arriving earlier rather than a
/// second opinion. A failure to resolve is not reported here: the command
/// itself is about to fail on it, and saying so twice would put the operator's
/// own mistake in front of them as though it had happened twice.
///
/// Silent under `--json`, where the plan comes back inside the one document the
/// command returns. A script reading a stream of objects is owed one per run.
/// The command to run, once the operator has been told what it will affect.
///
/// The two directions share this because they share the sentence — only the verb
/// differs — and a second copy of "say it, then do it" would be a second place for
/// them to fall out of step about which half comes first.
/// A restart of named services, or of everything the form holds where none are named.
fn restarting(form: String, services: Vec<String>) -> Command {
    Command::Restart {
        forms: vec![form],
        services,
    }
}

/// The category a diagnosis was narrowed to, or the code to exit with for a name
/// that is not one.
///
/// A named category lemonfiber does not know is a mistake to correct rather than a
/// request to run everything — refused here, before the core is reached.
fn narrowed(only: Option<&str>) -> Result<Option<Category>, ExitCode> {
    match only.map(Category::parse) {
        Some(None) => {
            let named = only.unwrap_or_default();
            complain!("error: no diagnostic category named `{named}`");
            Err(ExitCode::from(USAGE))
        }
        Some(Some(category)) => Ok(Some(category)),
        None => Ok(None),
    }
}

/// Announce what starting will affect, then start it, narrated as it goes.
///
/// Starting does not go through dispatch, for the same reason a pull and a watch do
/// not: Compose narrates for minutes and the report comes at the end, which is not a
/// value that arrives once.
async fn starting(ctx: &Ctx, forms: &[String], services: &[String], json: bool) -> ExitCode {
    // Not announced where services are named. The announcement is about what a form
    // holds, and saying "starts eight services" before starting two of them would be
    // a sentence about a set the operator did not ask for.
    if services.is_empty() {
        announce(ctx, forms, json, Doing::Starting).await;
    }
    start(ctx, forms, services, json).await
}

/// Announce what stopping would affect, settle what to do about anything still
/// coming down, and only then ask for the stop.
///
/// Both happen before the teardown rather than during it: an operator who is going to
/// be told a download is at ninety per cent wants to be told while stopping is still
/// a question, not while it is already happening.
async fn halting(
    ctx: &Ctx,
    forms: Vec<String>,
    services: Vec<String>,
    wait: bool,
    yes: bool,
    json: bool,
) -> Command {
    // Not announced where services are named, for the same reason starting is not:
    // the announcement is about what a form holds, and naming two services is not a
    // request about the form.
    if services.is_empty() {
        announce(ctx, &forms, json, Doing::Stopping).await;
    }
    // Asked only of a whole teardown. What is in flight is a question about the
    // download clients a form holds, and stopping two named services that are not
    // download clients would report downloads that stopping them cannot interrupt.
    //
    // Machine-readable runs are left alone either way. A prompt has nobody to answer
    // it, and a report not in the envelope is noise on a stream something is parsing.
    if !json && services.is_empty() {
        settle(ctx, &forms, wait, yes).await;
    }
    // Stopping named services and tearing a form down are different requests rather
    // than one request with an argument, and Compose spells them differently too.
    if services.is_empty() {
        Command::Down { forms }
    } else {
        Command::Halt { forms, services }
    }
}

async fn announce(ctx: &Ctx, forms: &[String], json: bool, doing: Doing) {
    if json {
        return;
    }
    if let Ok(Outcome::Preview(plan)) = dispatch(
        Command::Preview {
            forms: forms.to_vec(),
        },
        ctx,
    )
    .await
    {
        render::stack::affects(&plan, doing).print();
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
async fn setup_from(ctx: Ctx, raw: RawSetup) -> ExitCode {
    let flags = match SetupFlags::parse(raw) {
        Ok(flags) => flags,
        Err(message) => {
            complain!("error: {message}");
            return ExitCode::from(USAGE);
        }
    };
    let Some(paths) = here() else {
        return no_config_home();
    };
    setting_up(ctx, &paths, &Console, flags).await
}
