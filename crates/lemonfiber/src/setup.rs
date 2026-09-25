//! The first-run setup surface — the conversation, not the decisions.
//!
//! A bare `lemonfiber` and `lemonfiber setup` both land here: this greets an
//! operator, offers or resumes or recovers a setup, drives the wizard against a
//! terminal or the flags, and brings the stack up once the answers are applied.
//! What to ask, what an answer means, and what to write are all the core's,
//! reached through [`core_setup`]; reading a line and rendering a question are the
//! surface's, and that is what lives here. Split out of `main` so the dispatcher
//! stays a dispatcher.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use lemonfiber_core::app::apply::Applying;
use lemonfiber_core::app::{setup as core_setup, Ctx};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::kind;
use lemonfiber_core::model::{Envelope, SetupOutcome, SetupReport};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::wizard::{offer_setup, Progress, Status, Wizard};
use lemonfiber_core::PRODUCT;

mod boot;
mod door;
mod first_content;
mod interrupted;

use boot::{preflight, start};
use interrupted::recover_setup;

use crate::context::read_settings;
use crate::exit::{complain, USAGE};
use crate::prompt::{Flags, SetupFlags};
use crate::say::{complain, say};

/// The three ways setup reaches a person: whether one is there, what they typed,
/// and what does the asking.
///
/// Behind a trait because each is a thing no test can be. What setup *decides* —
/// which of the ways out of an interrupted run to offer, whether a machine is
/// already configured, what to do with an answer — is all on this side of it, and
/// is the part worth holding to anything.
pub(crate) trait Surface {
    /// Whether anyone is present to answer.
    fn interactive(&self) -> bool;

    /// Whether there is a screen to draw on.
    ///
    /// A separate question from whether anyone can answer, because the two streams
    /// are separately redirected: `lemonfiber > out.txt` leaves a keyboard attached
    /// and no screen. Asked of every surface rather than defaulted to the other, so
    /// a surface that has not thought about it does not quietly answer for both.
    fn drawable(&self) -> bool;

    /// Show a prompt and read the trimmed line typed in reply.
    fn line(&self, prompt: &str) -> String;

    /// What puts setup's questions, once there is someone to put them to.
    fn asking(
        &self,
        environment: Environment,
        default_data: PathBuf,
    ) -> Box<dyn core_setup::Prompt>;
}

/// Say what the run came to, in the shape whoever asked for it can use.
///
/// One place, so a run that can end three ways cannot answer in three shapes.
fn concluded(outcome: SetupOutcome, settings: &Settings, prose: &[String], parsed: bool) {
    if parsed {
        document(outcome, settings).print();
        return;
    }
    for line in prose {
        say!("{line}");
    }
}

/// The same conclusion, for something that will parse it.
///
/// Built from the settings as they stand once the run is over, so what a script is
/// told is what the machine now holds rather than what was typed at it.
fn document(outcome: SetupOutcome, settings: &Settings) -> crate::render::Lines {
    let report = SetupReport {
        outcome,
        protocols: settings.protocols,
        data_root: settings.data_root.clone(),
        service_user: settings
            .service_user
            .map(|(user, group)| format!("{user}:{group}")),
    };
    let mut lines = crate::render::Lines::for_a_parser();
    lines.put(
        Envelope::new(kind::SETUP, &report)
            .to_json()
            .unwrap_or(crate::render::UNRENDERABLE.to_owned()),
    );
    lines
}

/// Where to send an operator whose machine is already set up, for a run nobody is
/// watching.
///
/// The words rather than the printing, so what a bare run says is proven here and
/// only the act of saying it happens at the edge.
pub(crate) fn already_set_up() -> Vec<String> {
    vec![
        format!("{PRODUCT} is already set up on this machine."),
        format!("  · change a setting with `{PRODUCT} config set <key> <value>`"),
        format!("  · start the stack with `{PRODUCT} up`"),
    ]
}

/// What a bare run on a machine that is already set up does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Bare {
    /// Open the dashboard — what a bare invocation means when somebody is watching.
    Dashboard,
    /// Say where to go next, for a run nobody is watching.
    Guidance,
}

/// Which of the two a bare run on a configured machine is.
///
/// The dashboard holds the terminal until somebody leaves it, which is right in
/// front of a person and wrong in a pipe, a cron line or a CI step — there it
/// would draw to nothing and never return. So the one case that cannot be tested
/// (a real terminal) is reached through a decision that can be.
pub(crate) const fn bare_run(interactive: bool) -> Bare {
    if interactive {
        Bare::Dashboard
    } else {
        Bare::Guidance
    }
}

/// The greeting itself, once there is somewhere to keep files and something to
/// hold the conversation across.
pub(crate) async fn greeting(ctx: Ctx, paths: &Paths, surface: &dyn Surface) -> ExitCode {
    // Before anything is read, because every branch below this one leads somewhere
    // that applies answers — including the unfinished-setup branch, which used to be
    // taken first and walked a rehearsal straight into writing settings.
    if ctx.dry_run {
        return nothing_to_rehearse();
    }

    // A stopped apply or a quit mid-question is unfinished setup, not a fresh or a
    // finished machine, and must be caught before the configured-yet check below —
    // an interrupted apply leaves half-written settings that check would read as
    // done. Handing these to the setup path detects and offers the way out.
    let progress = core_setup::progress_at(&paths.setup_progress());
    if Status::of(progress.as_ref()).unfinished() {
        // A bare invocation carries no flags; unfinished setup is picked up
        // interactively, the same conversation a fresh bare run would have.
        return setting_up(ctx, paths, surface, SetupFlags::none()).await;
    }

    if !offer_setup(paths.env_file().exists()) {
        // Already set up: setup would walk a done machine back to its first
        // question, so a bare run does the other thing it could mean.
        return crate::terminal::configured(ctx, bare_run(surface.drawable())).await;
    }

    say!("No configuration found.");

    if !surface.interactive() {
        // No one is here to take the offer, so it is stated rather than asked —
        // never left waiting on input that will not come.
        say!("Run `{PRODUCT} setup` to configure your stack.");
        return ExitCode::SUCCESS;
    }
    if !confirm_setup(surface) {
        say!("No changes made — run `{PRODUCT} setup` when you are ready.");
        return ExitCode::SUCCESS;
    }
    setting_up(ctx, paths, surface, SetupFlags::none()).await
}

/// Ask whether to begin setup now, taking silence and anything but a clear no as
/// yes — a first run is what a bare invocation on an unconfigured machine means,
/// so the gentle default is to proceed.
fn confirm_setup(surface: &dyn Surface) -> bool {
    !matches!(
        surface
            .line("Run first-time setup? [Y/n]")
            .to_lowercase()
            .as_str(),
        "n" | "no"
    )
}

/// What setup says when it is asked to rehearse.
///
/// Setup is the one conversation that *is* the change: what it would apply is what
/// the operator has not typed yet, so a rehearsal of it would be a report of nothing
/// followed by the same questions again. That is the shape a walkthrough refuses the
/// flag under, and it belongs to the conversation rather than to setup: the steps a
/// surface drives one at a time — where it stands, an answer recorded, the apply — are
/// values that arrive once, and every one of those says what it would write and writes
/// none of it. This path does not go through the dispatcher, so the refusal is said
/// here, and it points at the two commands that do answer.
pub(crate) fn nothing_to_rehearse() -> ExitCode {
    complain!("Setup asks its questions as it goes, so there is nothing yet to rehearse.");
    complain!(
        "Run `{PRODUCT} setup --status` to see where it stands and what applying would write."
    );
    complain!("Run `{PRODUCT} setup` without --dry-run when you are ready.");
    ExitCode::from(USAGE)
}

/// What a previous run left decides what this one does — the whole of setup's
/// routing, once there is somewhere to keep files and something to ask across.
pub(crate) async fn setting_up(
    ctx: Ctx,
    paths: &Paths,
    surface: &dyn Surface,
    flags: SetupFlags,
) -> ExitCode {
    // The funnel both entry points reach, so the refusal is here as well as at the
    // greeting: a run given flags on the command line never passes the greeting at
    // all, and would otherwise apply them.
    if ctx.dry_run {
        return nothing_to_rehearse();
    }
    // What a previous run left decides what this one does. An apply that stopped
    // part-way is offered back before anything else — otherwise the configured-yet
    // check below would see its half-written settings and call the machine done.
    let progress = core_setup::progress_at(&paths.setup_progress());
    match Status::of(progress.as_ref()) {
        Status::FailedApply => recover_setup(ctx, paths, surface, progress).await,
        // A run that quit mid-question saved where it reached; pick it back up.
        Status::InProgress => resume_gather(ctx, paths, surface, progress, flags).await,
        // Absent and applied are neither a stopped apply nor a saved run, so they
        // begin, or decline, a fresh one.
        _ => fresh_setup(ctx, paths, surface, flags).await,
    }
}

/// Gather answers on a machine with nothing to recover or resume.
async fn fresh_setup(
    ctx: Ctx,
    paths: &Paths,
    surface: &dyn Surface,
    flags: SetupFlags,
) -> ExitCode {
    // Setup is for a machine with nothing configured; a configured one is changed
    // through its settings, not walked back to its first question.
    if !offer_setup(paths.env_file().exists()) {
        concluded(
            SetupOutcome::AlreadySetUp,
            &ctx.settings,
            &[
                "This machine is already set up.".to_owned(),
                format!("Change a setting with `{PRODUCT} config set`, or start it with `{PRODUCT} up`."),
            ],
            crate::say::for_a_parser(),
        );
        return ExitCode::from(USAGE);
    }

    let environment = ctx.environment;
    drive(ctx, paths, surface, Wizard::new(environment), flags).await
}

/// Pick a setup back up from the answers a quit run saved.
async fn resume_gather(
    ctx: Ctx,
    paths: &Paths,
    surface: &dyn Surface,
    progress: Option<Progress>,
    flags: SetupFlags,
) -> ExitCode {
    // In-progress means a saved run; if it is somehow gone there is nothing to
    // resume, so a fresh run is the honest fallback.
    let Some(progress) = progress else {
        return fresh_setup(ctx, paths, surface, flags).await;
    };
    say!("Picking up where a previous setup left off.");
    let environment = ctx.environment;
    drive(
        ctx,
        paths,
        surface,
        Wizard::resume(environment, progress),
        flags,
    )
    .await
}

/// Ask the questions the `wizard` still needs, apply the answers, and start.
///
/// The answers come from a terminal where there is one; where there is not, they
/// come from the flags. Either way it is the same walk — the wizard cannot tell —
/// so a flag run still probes the data location and proves the indexer, with the
/// warnings a person would weigh settled by the standing `--yes`.
async fn drive(
    mut ctx: Ctx,
    paths: &Paths,
    surface: &dyn Surface,
    mut wizard: Wizard,
    flags: SetupFlags,
) -> ExitCode {
    // The environment is checked before the first question, so a missing or
    // unreachable container engine is caught here rather than after eleven
    // answers — nothing setup does can work without one.
    if let Err(code) = preflight(&ctx).await {
        return code;
    }

    // Credentials are proven against their live services as they are entered — the
    // indexer and any existing service over HTTP, a Usenet provider over a real,
    // TLS-wrapped NNTP connection. The context carries the one that does it, so a
    // run driven a question at a time from a browser proves them the same way.
    let validator = Arc::clone(&ctx.validator);

    // A terminal answers the questions; without one the flags do, and where a flag
    // a question needs is missing the run is told which rather than left waiting on
    // input that never comes.
    let prompt: Box<dyn core_setup::Prompt> = if surface.interactive() {
        surface.asking(ctx.environment, default_data_location(paths))
    } else {
        let missing = flags.missing(&wizard);
        if !missing.is_empty() {
            complain!("error: setup here is non-interactive, so it needs values as flags:");
            for flag in missing {
                complain!("  {flag}");
            }
            complain!("\nRun it in a terminal to answer interactively instead.");
            return ExitCode::from(USAGE);
        }
        Box::new(Flags::new(flags, default_data_location(paths)))
    };

    let at = stamp();
    let applying = Applying {
        paths,
        source: ctx.stack,
        stamp: &at,
        random: ctx.seams.random.as_ref(),
    };
    match core_setup::run(
        &mut wizard,
        prompt.as_ref(),
        ctx.seams.filesystem.as_ref(),
        validator.as_ref(),
        &applying,
    )
    .await
    {
        Ok(core_setup::Outcome::Applied) => {
            // The settings read at startup predate the file setup just wrote, so
            // they are refreshed before the stack is brought up against them.
            ctx.settings = read_settings();
            concluded(
                SetupOutcome::Applied,
                &ctx.settings,
                &["\nSetup is done — bringing your stack up.".to_owned()],
                crate::say::for_a_parser(),
            );
            start(&ctx, surface).await
        }
        Ok(core_setup::Outcome::Abandoned) => {
            concluded(
                SetupOutcome::Abandoned,
                &ctx.settings,
                &["\nSetup was left here — nothing was written.".to_owned()],
                crate::say::for_a_parser(),
            );
            ExitCode::SUCCESS
        }
        Err(problem) => complain(&problem),
    }
}

/// The data location setup proposes when the operator does not name one.
///
/// A directory under this machine's data base, so a default run lands somewhere
/// real and writable; an operator with a NAS or a separate disk names that
/// instead.
fn default_data_location(paths: &Paths) -> PathBuf {
    paths.data_dir().join("media")
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

#[cfg(test)]
pub(crate) mod tests;
