//! The first-run setup surface — the conversation, not the decisions.
//!
//! A bare `lemonfiber` and `lemonfiber setup` both land here: this greets an
//! operator, offers or resumes or recovers a setup, drives the wizard against a
//! terminal or the flags, and brings the stack up once the answers are applied.
//! What to ask, what an answer means, and what to write are all the core's,
//! reached through [`core_setup`]; reading a line and rendering a question are the
//! surface's, and that is what lives here. Split out of `main` so the dispatcher
//! stays a dispatcher.

use std::io::IsTerminal as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use lemonfiber_core::app::{dispatch, recover, setup as core_setup, Command, Ctx, Outcome};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::doctor::{Category, Overall};
use lemonfiber_core::journal::{Change, Kind};
use lemonfiber_core::validate::Live;
use lemonfiber_core::wizard::{
    offer_setup, Choice, Progress, Recovery, Resolution, Status, Wizard,
};
use lemonfiber_core::PRODUCT;

use crate::prompt::{Flags, SetupFlags, Terminal};
use crate::read_line;
use crate::render::render;
use crate::{
    complain, context, here, pull_showing, read_settings, settled, FAILURE, PREFLIGHT, USAGE,
};

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
pub(crate) async fn greet(stack_dir: Option<PathBuf>, dry_run: bool) -> ExitCode {
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
    let progress = core_setup::progress_at(&paths.setup_progress());
    if matches!(
        Status::of(progress.as_ref()),
        Status::FailedApply | Status::InProgress
    ) {
        // A bare invocation carries no flags; unfinished setup is picked up
        // interactively, the same conversation a fresh bare run would have.
        return run_setup(ctx, SetupFlags::none()).await;
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
    run_setup(ctx, SetupFlags::none()).await
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
/// and what to write are all the core's, reached through [`core_setup::run`].
pub(crate) async fn run_setup(ctx: Ctx, flags: SetupFlags) -> ExitCode {
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
    let progress = core_setup::progress_at(&paths.setup_progress());
    match Status::of(progress.as_ref()) {
        Status::FailedApply => recover_setup(ctx, &paths, progress).await,
        // A run that quit mid-question saved where it reached; pick it back up.
        Status::InProgress => resume_gather(ctx, &paths, progress, flags).await,
        // Absent and applied are neither a stopped apply nor a saved run, so they
        // begin, or decline, a fresh one.
        _ => fresh_setup(ctx, &paths, flags).await,
    }
}

/// Gather answers on a machine with nothing to recover or resume.
async fn fresh_setup(ctx: Ctx, paths: &Paths, flags: SetupFlags) -> ExitCode {
    // Setup is for a machine with nothing configured; a configured one is changed
    // through its settings, not walked back to its first question.
    if !offer_setup(paths.env_file().exists()) {
        println!("This machine is already set up.");
        println!("Change a setting with `{PRODUCT} config set`, or start it with `{PRODUCT} up`.");
        return ExitCode::from(USAGE);
    }

    let environment = ctx.environment;
    drive(ctx, paths, Wizard::new(environment), flags).await
}

/// Pick a setup back up from the answers a quit run saved.
async fn resume_gather(
    ctx: Ctx,
    paths: &Paths,
    progress: Option<Progress>,
    flags: SetupFlags,
) -> ExitCode {
    // In-progress means a saved run; if it is somehow gone there is nothing to
    // resume, so a fresh run is the honest fallback.
    let Some(progress) = progress else {
        return fresh_setup(ctx, paths, flags).await;
    };
    println!("Picking up where a previous setup left off.");
    let environment = ctx.environment;
    drive(ctx, paths, Wizard::resume(environment, progress), flags).await
}

/// Ask the questions the `wizard` still needs, apply the answers, and start.
///
/// The answers come from a terminal where there is one; where there is not, they
/// come from the flags. Either way it is the same walk — the wizard cannot tell —
/// so a flag run still probes the data location and proves the indexer, with the
/// warnings a person would weigh settled by the standing `--yes`.
async fn drive(mut ctx: Ctx, paths: &Paths, mut wizard: Wizard, flags: SetupFlags) -> ExitCode {
    // The environment is checked before the first question, so a missing or
    // unreachable container engine is caught here rather than after eleven
    // answers — nothing setup does can work without one.
    if let Err(code) = preflight(&ctx).await {
        return code;
    }

    // Credentials are proven against their live services as they are entered — the
    // indexer and any existing service over HTTP, a Usenet provider over a real,
    // TLS-wrapped NNTP connection.
    let validator = Live::with_nntp(
        ctx.http.clone(),
        Arc::new(lemonfiber_core::adapters::Dialer::new()),
    );

    // A terminal answers the questions; without one the flags do, and where a flag
    // a question needs is missing the run is told which rather than left waiting on
    // input that never comes.
    let prompt: Box<dyn core_setup::Prompt> = if std::io::stdin().is_terminal() {
        Box::new(Terminal::new(ctx.environment, default_data_location()))
    } else {
        let missing = flags.missing(&wizard);
        if !missing.is_empty() {
            eprintln!("error: setup here is non-interactive, so it needs values as flags:");
            for flag in missing {
                eprintln!("  {flag}");
            }
            eprintln!("\nRun it in a terminal to answer interactively instead.");
            return ExitCode::from(USAGE);
        }
        Box::new(Flags::new(flags, default_data_location()))
    };

    match core_setup::run(
        &mut wizard,
        prompt.as_ref(),
        ctx.filesystem.as_ref(),
        &validator,
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
            println!("\nSetup is done — bringing your stack up.");
            start(&ctx).await
        }
        Ok(core_setup::Outcome::Abandoned) => {
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
    // nothing to resume from, so a fresh run is the honest fallback — interactive,
    // since recovery carries no flags.
    let Some(progress) = progress else {
        return fresh_setup(ctx, paths, SetupFlags::none()).await;
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
    match core_setup::resume(&mut wizard, paths, ctx.stack, &stamp()) {
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
///
/// The images are pulled first, with their progress on screen, so the several
/// gigabytes come down where the operator can watch rather than as a silent wait
/// inside `up`. Only once they are down is the stack brought up and waited on for
/// health; a pull that failed stops here rather than starting against images that
/// never arrived.
async fn start(ctx: &Ctx) -> ExitCode {
    let forms = vec![STARTER_FORM.to_owned()];
    if let Err(code) = pull_showing(ctx, &forms, false).await {
        return code;
    }

    match dispatch(Command::Up { forms }, ctx).await {
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
