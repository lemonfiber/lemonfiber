//! The first-run setup surface — the conversation, not the decisions.
//!
//! A bare `lemonfiber` and `lemonfiber setup` both land here: this greets an
//! operator, offers or resumes or recovers a setup, drives the wizard against a
//! terminal or the flags, and brings the stack up once the answers are applied.
//! What to ask, what an answer means, and what to write are all the core's,
//! reached one step at a time through the dispatcher, the same steps a browser
//! takes; reading a line and rendering a question are the surface's, and that is
//! what lives here. Split out of `main` so the dispatcher stays a dispatcher.

use std::path::PathBuf;
use std::process::ExitCode;

use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::{SetUp, SetupOutcome, WizardReport};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::wizard::Phase;
use lemonfiber_core::PRODUCT;

mod boot;
mod door;
mod first_content;
mod interrupted;

use boot::{preflight, start};
use interrupted::recover_setup;

use crate::context::settings_at;
use crate::exit::{complain, USAGE};
use crate::prompt::{Prompt, SetupFlags};
use crate::say::{complain, say};
use lemonfiber::setting_up::{self, default_data_location, Walked};

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
    fn asking(&self, environment: Environment, default_data: PathBuf) -> Box<dyn Prompt>;
}

/// Where setup stands on this machine, as the walk's first step reads it.
async fn standing(ctx: &Ctx) -> Result<WizardReport, ExitCode> {
    match setting_up::standing(ctx).await {
        Ok(report) => report.ok_or(ExitCode::FAILURE),
        Err(problem) => Err(complain(&problem)),
    }
}

/// Whether a previous run left setup part-way: answers to pick up, a review to
/// confirm, or an apply that stopped.
///
/// Asked before whether the machine is configured: an apply that stopped part-way
/// has written settings that question reads as a finished install.
fn unfinished(report: &WizardReport) -> bool {
    report.setup == SetUp::PartWay
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
    let mut lines = crate::render::Lines::for_a_parser();
    lines.put(
        setting_up::report(outcome, settings)
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
    // that applies answers — including the unfinished-setup branch, which would walk
    // a rehearsal straight into writing settings.
    if ctx.dry_run {
        return nothing_to_rehearse();
    }

    // A stopped apply or a quit mid-question is unfinished setup, not a fresh or a
    // finished machine, and must be caught before the configured-yet check below —
    // an interrupted apply leaves half-written settings that check would read as
    // done. Handing these to the setup path detects and offers the way out.
    let report = match standing(&ctx).await {
        Ok(report) => report,
        Err(code) => return code,
    };
    if unfinished(&report) {
        return setting_up(ctx, paths, surface, SetupFlags::none()).await;
    }

    if !report.offered {
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
    let report = match standing(&ctx).await {
        Ok(report) => report,
        Err(code) => return code,
    };
    if !unfinished(&report) {
        return Box::pin(fresh_setup(ctx, paths, surface, flags, report)).await;
    }
    if report.phase == Phase::Applying {
        return Box::pin(recover_setup(ctx, paths, surface, report.written)).await;
    }
    say!("Picking up where a previous setup left off.");
    Box::pin(drive(ctx, paths, surface, report, flags)).await
}

/// Gather answers on a machine with nothing to recover or resume.
async fn fresh_setup(
    ctx: Ctx,
    paths: &Paths,
    surface: &dyn Surface,
    flags: SetupFlags,
    report: WizardReport,
) -> ExitCode {
    if !report.offered {
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
    drive(ctx, paths, surface, report, flags).await
}

/// Ask the questions setup still needs, apply the answers, and start.
///
/// The answers come from a terminal where there is one; where there is not, they
/// come from the flags. Either way it is the same walk, so a flag run still has the
/// data location tested and the indexer proven, with the warnings a person would
/// weigh settled by the standing `--yes`.
async fn drive(
    mut ctx: Ctx,
    paths: &Paths,
    surface: &dyn Surface,
    report: WizardReport,
    flags: SetupFlags,
) -> ExitCode {
    if let Err(code) = preflight(&ctx).await {
        return code;
    }

    let prompt: Box<dyn Prompt> = if surface.interactive() {
        surface.asking(ctx.environment, default_data_location(paths))
    } else {
        match setting_up::answering(&report.unanswered, flags, paths) {
            Ok(flags) => Box::new(flags),
            Err(missing) => {
                complain!("error: setup here is non-interactive, so it needs values as flags:");
                for flag in missing {
                    complain!("  {flag}");
                }
                complain!("\nRun it in a terminal to answer interactively instead.");
                return ExitCode::from(USAGE);
            }
        }
    };

    match setting_up::walk(&ctx, prompt.as_ref(), report).await {
        Ok(Walked::Applied) => {
            ctx.settings = settings_at(Some(&ctx), Some(paths));
            concluded(
                SetupOutcome::Applied,
                &ctx.settings,
                &["\nSetup is done — bringing your stack up.".to_owned()],
                crate::say::for_a_parser(),
            );
            Box::pin(start(&ctx, surface)).await
        }
        Ok(Walked::Abandoned) => {
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

#[cfg(test)]
pub(crate) mod tests;
