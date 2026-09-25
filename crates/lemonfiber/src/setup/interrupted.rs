//! The ways out of a setup that stopped part-way.
//!
//! An interrupted apply is the one state setup must never guess about: it wrote
//! something, and what it wrote is the operator's to keep, undo, or forget. Kept
//! apart from the gathering so the three ways out read as the whole of the choice.
//!
//! What is here is the offering and nothing else. Which changes were written comes
//! back from the same read a browser makes, and each way out is one of the core's
//! own commands — so the choice an operator is put is the choice a browser is put,
//! and the work behind it is one implementation rather than two.

use std::process::ExitCode;

use lemonfiber_core::app::{dispatch, setup as core_setup, Command, Ctx, SetupAction};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::wizard::{Choice, Progress};
use lemonfiber_core::PRODUCT;

use super::boot::start;
use super::{fresh_setup, Surface};
use crate::context::read_settings;
use crate::exit::{complain, USAGE};
use crate::prompt::SetupFlags;
use crate::say::{complain, say};

/// Offer the operator a way out of a setup whose apply stopped part-way.
///
/// It is shown what the interrupted run wrote and given the three ways forward the
/// wizard keeps recoverable: finish it, undo and redo it, or undo and forget it.
/// Deciding is not done for a piped run that cannot answer — the state is left as
/// it is, still recoverable, rather than acted on unasked.
pub(super) async fn recover_setup(
    mut ctx: Ctx,
    paths: &Paths,
    surface: &dyn Surface,
    progress: Option<Progress>,
) -> ExitCode {
    // A stopped apply always leaves its answers; if they are somehow gone there is
    // nothing to resume from, so a fresh run is the honest fallback — interactive,
    // since recovery carries no flags.
    if progress.is_none() {
        return fresh_setup(ctx, paths, surface, SetupFlags::none()).await;
    }

    say!("A previous setup was interrupted part-way through applying.");
    // The same list a browser is shown before it is offered the same three ways
    // out, in the same words, because both read it from the one place.
    let written = core_setup::written_so_far(paths);
    if written.is_empty() {
        say!("It had not written anything yet.");
    } else {
        say!("It had written:");
        for change in &written {
            say!("  · {change}");
        }
    }

    if !surface.interactive() {
        complain!("\nerror: recovering an interrupted setup needs a terminal to choose.");
        complain!("Run `{PRODUCT} setup` interactively to resume, roll back, or start over.");
        return ExitCode::from(USAGE);
    }

    let choice = ask_recovery_choice(surface);
    // Said before rather than after: the undo and the apply behind a roll back are
    // one step now, so there is no moment between them to report from — and what is
    // about to happen is what an operator watching a pause wants to read.
    say!("\n{}", about_to(choice));
    if let Err(problem) = dispatch(Command::Setup(SetupAction::Recover(choice)), &ctx).await {
        return complain(&problem);
    }

    if choice == Choice::StartOver {
        say!("Nothing of the interrupted setup remains.");
        say!("Run `{PRODUCT} setup` to begin again.");
        return ExitCode::SUCCESS;
    }
    // The settings read at startup predate the file the recovery just wrote, so
    // they are refreshed before the stack is brought up against them.
    ctx.settings = read_settings();
    say!("\nSetup is done — bringing your stack up.");
    start(&ctx, surface).await
}

/// What each way out is about to do, said before it happens.
fn about_to(choice: Choice) -> &'static str {
    match choice {
        Choice::Resume => "Resuming — finishing the apply from where it stopped.",
        Choice::RollBack => "Rolling back what was written, then applying again.",
        Choice::StartOver => "Undoing what was written and forgetting the answers.",
    }
}

/// Which way out of an interrupted setup the operator chooses.
pub(super) fn ask_recovery_choice(surface: &dyn Surface) -> Choice {
    say!("\nWhat would you like to do?");
    say!("  1) Resume — finish applying from where it stopped");
    say!("  2) Roll back — undo what was written, then apply again");
    say!("  3) Start over — undo it and forget the answers");
    match surface.line("Choose [1]:").as_str() {
        "2" => Choice::RollBack,
        "3" => Choice::StartOver,
        _ => Choice::Resume,
    }
}

#[cfg(test)]
mod tests;
