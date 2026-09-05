//! The `lemonfiber` binary — the surfaces, over one core.
//!
//! This crate is the only one that renders. It turns input into a command,
//! hands it to the core, and renders what comes back; it makes no decisions of
//! its own, which is why the same request behaves identically whether it arrived
//! as a subcommand, a keypress or an HTTP route.

use std::process::ExitCode;

use clap::Parser;
use lemonfiber::cli::{Cli, Mending, RawSetup, RawUi, Request};
use lemonfiber_core::app::restore::{Consent, Kept};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, SetupAction, Waiting};
use lemonfiber_core::doctor::Narrowing;

mod acting;
mod archive;
mod context;
mod dashboard;
mod engine;
mod exit;
mod keyboard;
mod logs;
mod pane;
mod prompt;
mod render;
mod repair;
mod say;
mod setup;
mod stopping;
mod terminal;
mod translate;
mod ui;

use crate::say::{complain, say};
use context::{context, here};
use engine::{pull, settle, start, stream};
use exit::{complain, no_config_home, settled, USAGE};
use keyboard::{Console, Keyboard};
use prompt::SetupFlags;
use render::render;
use render::stack::Doing;
use render::walkthrough::{Narrating as WalkNarrating, Quiet};
use setup::{greeting, setting_up};
use stopping::Choice;
use translate::{
    bundling, configuration, household, invitation, letting, quality, restarting, traced,
};

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

/// The one thing a walk was asked for, or nothing at all.
///
/// Taken as words so it can be typed unquoted, and joined back into the title as
/// said. Nothing named is a request in its own right rather than an omission: a
/// walk asked for nothing in particular suggests something likely to work, which
/// is what an operator with an empty library needs.
fn named(words: &[String]) -> Option<String> {
    let said = words.join(" ");
    (!said.trim().is_empty()).then_some(said)
}

/// Where a walk's steps go while it runs.
///
/// A run whose whole answer is a JSON document must not have prose interleaved into
/// it: a consumer parsing that stream would be handed something that is not a
/// document. So a machine-readable walk is narrated to nobody, and the report at
/// its end is the whole of what it says.
fn walking(json: bool) -> std::sync::Arc<dyn lemonfiber_core::walkthrough::Narrator> {
    if json {
        std::sync::Arc::new(Quiet)
    } else {
        std::sync::Arc::new(WalkNarrating)
    }
}

/// A restore, which is the same command asked twice.
///
/// The first asks what the archive holds and overwrites nothing; the second
/// overwrites. An operator at a shell is present for both, so this makes both and
/// prints the listing between them — which is the same pair of answers a browser
/// gets as two requests, with the operator's decision in the gap.
///
/// A first answer that refuses ends the run: there is nothing to be shown, and
/// asking again would only produce the same refusal after the operator had been
/// told once.
async fn restoring(ctx: &Ctx, archive: Kept, repoint: bool, json: bool) -> ExitCode {
    let looking = Command::Restore {
        archive: archive.clone(),
        repoint,
        consent: Consent::List,
    };
    match dispatch(looking, ctx).await {
        Ok(outcome) => render(&outcome, json),
        Err(problem) => return complain(&problem),
    }
    // Standing consent rather than a name for the listing just printed. The two
    // commands are one run over one look, so there is no gap for the archive to
    // move in — which is exactly what the operator who typed the agreement in
    // advance was agreeing to.
    let doing = Command::Restore {
        archive,
        repoint,
        consent: Consent::Standing,
    };
    match dispatch(doing, ctx).await {
        Ok(outcome) => {
            render(&outcome, json);
            settled(&outcome)
        }
        Err(problem) => complain(&problem),
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

/// A repairing run, which ends here rather than going through dispatch.
///
/// It looks, offers, acts and looks again, rendering what became of each — not one
/// value an envelope holds — so it answers for itself. With nowhere to keep its own
/// files there is nothing to repair against, which is the one thing it cannot work
/// around.
async fn repairing(ctx: Ctx, mending: Mending, json: bool) -> ExitCode {
    let Some(paths) = here() else {
        return no_config_home();
    };
    repair::run(ctx, paths, mending, &Keyboard, json).await
}

/// A context that narrates each step, as a walk-through wants and a script does not.
///
/// Named because it is the one piece of context-building long enough to push the arm
/// that needs it onto three lines, and `main` has no room to spare.
fn narrating(ctx: Ctx, json: bool) -> Ctx {
    ctx.narrating_steps(walking(json))
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

    let mut ctx = context(cli.stack_dir.take(), cli.dry_run, cli.force);

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
        // Long-running, and still a value that arrives once: what a guard produces
        // is one report, at the end. So it goes through dispatch like everything
        // that answers, and the waiting is the command's rather than this file's.
        Request::Watch { forms } => Command::Watch { forms },
        // Asking where setup stands is a value that arrives once, so unlike the
        // conversation below it goes through dispatch like every other question.
        Request::Setup { flags } if flags.status => Command::Setup(SetupAction::Where),
        // Setup itself is a conversation and then a stack coming up, not a value
        // that arrives once, so like streaming and watching it runs its own way.
        // It takes the context by value because it rewrites the settings mid-run.
        //
        // Narrated because setup ends by offering the walk: the offer is put at a
        // terminal, so what the walk says has a terminal to say it to.
        Request::Setup { flags } => return setup_from(narrating(ctx, cli.json), flags).await,
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
                return repairing(ctx, mending, cli.json).await;
            }
            match diagnosing(only, disruptive, accept) {
                Ok(command) => command,
                Err(code) => return code,
            }
        }
        Request::Trace {
            term,
            season,
            search,
        } => traced(&term, season, search),
        // Narrated for minutes and then one report, so the report goes through
        // dispatch and the narration goes wherever the surface is listening. A run
        // whose whole answer is a JSON document must not have prose interleaved
        // into it, so a machine-readable run listens with nobody.
        //
        // The term is taken as words so it can be typed unquoted; joined back into
        // the title as said, and nothing named at all asks to be suggested
        // something.
        Request::Walkthrough { item } => {
            ctx = ctx.narrating_steps(walking(cli.json));
            Command::Walkthrough { item: named(&item) }
        }
        // Naming a word says what it means and naming none lists them, and both are
        // answered from a table compiled into the binary rather than from a stack.
        Request::Explain { word } => return explaining(&ctx, &word, cli.json, cli.dry_run).await,
        Request::Household { member, action } => match household(member, action) {
            Ok(command) => command,
            Err(code) => return ExitCode::from(code),
        },
        Request::Stuck => Command::Stuck,
        Request::FrontDoor => Command::FrontDoor,
        Request::Outbound => Command::Outbound,
        Request::Stored => Command::Stored,
        Request::Clients => Command::Clients,
        Request::Invite { name, allowance } => invitation(name, allowance),
        Request::Reissue { name } => Command::Reissue { name },
        Request::Remove { name, confirm } => Command::Remove { name, confirm },
        Request::Forget { confirm } => Command::Forget { confirm },
        Request::Space { confirm } => Command::Space { confirm },
        Request::StopSeeding { download, offer } => letting(download, offer),
        Request::Bandwidth(asked) => translate::sharing(asked),
        Request::Seed => Command::Seed,
        Request::Adopt => Command::Adopt,
        Request::Reset { confirm } => Command::Reset { confirm },
        Request::Backup { service } => Command::Backup { service },
        Request::Support(asked) => bundling(asked),
        // The web surface holds the process until it is stopped, and answers many
        // requests rather than producing one value, so like the dashboard and the
        // log viewer it runs its own way instead of through dispatch. It takes the
        // context by value because every request it answers reaches through it.
        Request::Ui(asked) => return serving(ctx, asked).await,
        // A restore is the one request that is the same command twice: what it
        // would overwrite, and then the overwrite. Both are dispatched, so what
        // an operator is shown before it happens is the command's own answer
        // rather than this surface's rendering of one.
        //
        // Naming no archive asks which there are, the fork `forms` and `explain`
        // take on their own word: a surface that has to name one cannot know the
        // names in advance, and the browser cannot look in the directory at all.
        Request::Restore {
            archive: Some(archive),
            repoint,
        } => return restoring(&ctx, Kept::At(archive), repoint, cli.json).await,
        Request::Restore { archive: None, .. } => Command::Archives,
    };

    answered(command, &ctx, cli.json).await
}

/// The app this binary serves a browser.
///
/// Declared beside the stack in `cli`, so both embedded trees are named in one
/// place and a test outside this binary can read what it carries. See
/// `.docs/architecture/embedded-stack.md` for the shape it arrives in.
pub(crate) const EMBEDDED_APP: Option<lemonfiber_core::frontend::Source> = Some(
    lemonfiber_core::frontend::Source::Embedded(&lemonfiber::cli::APP),
);

/// Serve the web interface until the operator stops the process.
///
/// What ends the loop is a dependency rather than a choice made inside it, so the
/// surface can be started, asked something, and stopped, in a test. A real run is
/// handed a signal that never arrives.
async fn serving(ctx: Ctx, asked: RawUi) -> ExitCode {
    ui::run(
        ctx,
        asked.into(),
        &crate::keyboard::Keyboard,
        EMBEDDED_APP,
        Box::pin(std::future::pending()),
    )
    .await
}

/// Say what a word means, or list what there is to ask about, and record what this
/// operator has been told.
///
/// Kept apart from the fall-through in [`main`] by the record it writes: an
/// explanation a person read is recorded, and one a script fetched is not. A word
/// with no entry is refused by the core through the same error model as everything
/// else, so it carries a code and a way forward rather than being this one
/// command's private way of saying no.
///
/// The word is taken as words so it can be typed unquoted, and joined back into the
/// term as said.
async fn explaining(ctx: &Ctx, word: &[String], json: bool, rehearsing: bool) -> ExitCode {
    let said = word.join(" ");
    let command = if word.is_empty() {
        Command::Glossary
    } else {
        Command::Explain { word: said.clone() }
    };
    let outcome = match dispatch(command, ctx).await {
        Ok(outcome) => outcome,
        Err(problem) => return complain(&problem),
    };
    render(&outcome, json);
    // A rehearsal changes nothing, which this record is not exempt from. And a
    // script fetching the text is not a person learning the word: recording a
    // machine's lookup would quietly stop explaining it to the operator who never
    // made one. A listing teaches no one word, so there is none to record.
    if !word.is_empty() && !say::for_a_parser() {
        context::remember(&[said.as_str()], rehearsing);
    }
    settled(&outcome)
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
/// Carry the command out and say what came back.
///
/// The exit code is the outcome's own, so what a run reports and what it exits with
/// cannot disagree.
async fn answered(command: Command, ctx: &Ctx, json: bool) -> ExitCode {
    match dispatch(command, ctx).await {
        Ok(outcome) => {
            render(&outcome, json);
            settled(&outcome)
        }
        Err(problem) => complain(&problem),
    }
}

/// The diagnosis a plain run asks for, narrowed as it was asked to be.
///
/// Named apart because the arm it came from carries a fork of its own — a run that
/// mends returns before this is reached — and the two together are longer than the
/// table has room for.
fn diagnosing(
    only: Option<String>,
    disruptive: bool,
    accept: Option<String>,
) -> Result<Command, ExitCode> {
    narrowed(only.as_deref()).map(|narrowing| Command::Doctor {
        narrowing,
        disruptive,
        accept,
    })
}

/// What a diagnosis was narrowed to, or the code to exit with for a name that is
/// neither a category nor a check inside one.
///
/// A name lemonfiber does not know is a mistake to correct rather than a request to
/// run everything — refused here, before the core is reached. Whether a stack reports
/// the check named is a question only the run can answer, and it answers it.
fn narrowed(only: Option<&str>) -> Result<Narrowing, ExitCode> {
    match only.map(Narrowing::parse) {
        Some(None) => {
            let named = only.unwrap_or_default();
            complain!("error: no diagnostic category or check named `{named}`");
            Err(ExitCode::from(USAGE))
        }
        Some(Some(narrowing)) => Ok(narrowing),
        None => Ok(Narrowing::Suite),
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

/// Announce what stopping would affect, put the question about anything still coming
/// down, and hand the answer to the teardown.
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
    // Stopping named services and tearing a form down are different requests rather
    // than one request with an argument, and Compose spells them differently too.
    // The command line refuses the two flags together for the same reason.
    if !services.is_empty() {
        return Command::Halt { forms, services };
    }
    announce(ctx, &forms, json, Doing::Stopping).await;
    // Asked only where there is somebody to ask. A machine-readable run is put no
    // prompt — it has nobody to answer one, and a report not in the envelope is noise
    // on a stream something is parsing — so what it typed is what the teardown gets.
    let waiting = if json {
        wait
    } else {
        settle(ctx, &forms, wait, yes).await == Choice::Wait
    };
    Command::Down {
        forms,
        wait: Waiting::from(waiting),
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
