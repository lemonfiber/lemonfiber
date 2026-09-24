//! The offer setup ends on.
//!
//! Setup finishes at the moment of maximum uncertainty: everything is green and the
//! operator has no idea what to do. They installed this because they wanted to watch
//! something, and what has been delivered is infrastructure. So the last thing setup does
//! is offer to close that gap — and take no for an answer, completely, because a stack
//! that was set up is set up whether or not anyone watched it fetch something.
//!
//! A stack that could not honour the offer is not made one. Being asked "shall I fetch
//! something?" by a product that then cannot is worse than being told what is missing.

use std::process::ExitCode;

use lemonfiber_core::app::{dispatch, worth_offering, Command, Ctx};
use lemonfiber_core::docker::Condition;
use lemonfiber_core::walkthrough::{Shape, Why};
use lemonfiber_core::PRODUCT;

use super::Surface;
use crate::exit::{complain, settled as ended};
use crate::render::render;
use crate::say::say;

/// Offer the first-content walk, once the stack is up.
///
/// `condition` is what the stack settled to; a stack that is not fully up is left alone,
/// because setup has already said so and a walk over a half-started stack would fail in a
/// way that says nothing about the operator's machine.
pub(super) async fn offer(
    ctx: &Ctx,
    surface: &dyn Surface,
    condition: Option<Condition>,
    settled: ExitCode,
) -> ExitCode {
    if condition != Some(Condition::Active) {
        return settled;
    }
    match worth_offering(ctx).await {
        // Nothing to search with. Told rather than offered, because being asked and then
        // failing is the product demonstrating it does not know its own state.
        Ok(Why::Not(reason)) => {
            say!("\n{}", reason.said());
            say!("  → {}", reason.remedy());
            settled
        }
        Ok(Why::Offer(shape)) => ask(ctx, surface, shape, settled).await,
        // The offer is the last thing setup does and the least important; a stack that
        // cannot be read here has already been reported on by everything above.
        Err(_) => settled,
    }
}

/// Put the offer, and act on the answer.
async fn ask(ctx: &Ctx, surface: &dyn Surface, shape: Shape, settled: ExitCode) -> ExitCode {
    say!("\nOne more thing — {}.", shape.proves());
    if !surface.interactive() {
        // Nobody there to ask. Said as a thing they can do rather than done unasked: an
        // unattended run should not start fetching content on its own.
        say!("Run `{PRODUCT} walkthrough` when you are at a terminal.");
        return settled;
    }
    if !yes(surface) {
        // Declining carries no penalty, and saying so is the point: an operator who
        // thinks they have skipped something important will go looking for it.
        say!("Nothing lost — run `{PRODUCT} walkthrough` whenever you like.");
        return settled;
    }
    walked(ctx).await
}

/// Walk something, and report how it ended.
///
/// Nothing named: an operator who has just finished setup has an empty library, so
/// the walk suggests something likely to work rather than being told what to try.
/// Where each step is said is settled on the context this run was built with, which
/// for setup is the terminal the offer was put on.
async fn walked(ctx: &Ctx) -> ExitCode {
    match dispatch(Command::Walkthrough { item: None }, ctx).await {
        Ok(outcome) => {
            render(&outcome, false);
            ended(&outcome)
        }
        Err(problem) => complain(&problem),
    }
}

/// Whether the operator wants the walk. Silence means yes: they are at the end of a setup
/// they chose to run, and the walk is what they came for.
fn yes(surface: &dyn Surface) -> bool {
    !matches!(
        surface
            .line("Add one thing now and watch it work? [Y/n]:")
            .as_str(),
        "n" | "no" | "N" | "NO" | "No"
    )
}

#[cfg(test)]
mod tests;
