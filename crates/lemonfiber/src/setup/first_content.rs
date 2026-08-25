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
mod tests {
    use crate::exit::shown;
    use std::process::ExitCode;

    use lemonfiber_core::docker::Condition;

    use super::{offer, walked, yes};
    use crate::setup::tests::{working_ctx, Scripted};

    /// The code setup would have exited with, had there been no offer.
    fn already_settled() -> ExitCode {
        ExitCode::SUCCESS
    }

    #[tokio::test]
    async fn a_stack_that_is_not_fully_up_is_left_alone() {
        // Setup has already reported it, and a walk over a half-started stack fails in a
        // way that says nothing about the operator's machine.
        for condition in [None, Some(Condition::Partial), Some(Condition::Inactive)] {
            let ended = offer(
                &working_ctx(),
                &Scripted::saying(true, &[]),
                condition,
                already_settled(),
            )
            .await;
            assert_eq!(shown(ended), shown(already_settled()), "{condition:?}");
        }
    }

    #[tokio::test]
    async fn a_stack_with_nothing_to_search_is_told_rather_than_asked() {
        // Being asked "shall I fetch something?" by a product that then cannot is worse
        // than being told what is missing.
        let mut ctx = working_ctx();
        ctx.settings.protocols = lemonfiber_core::config::Protocols::both();
        let ended = offer(
            &ctx,
            &Scripted::saying(true, &[]),
            Some(Condition::Active),
            already_settled(),
        )
        .await;
        assert_eq!(shown(ended), shown(already_settled()));
    }

    #[tokio::test]
    async fn nobody_at_the_terminal_is_told_where_to_find_it_rather_than_walked() {
        // An unattended run should not start fetching content on its own.
        let ended = offer(
            &working_ctx(),
            &Scripted::saying(false, &[]),
            Some(Condition::Active),
            already_settled(),
        )
        .await;
        assert_eq!(shown(ended), shown(already_settled()));
    }

    #[tokio::test]
    async fn declining_leaves_setup_exactly_as_it_was() {
        // Declining carries no penalty. That is the whole promise, and the exit code is
        // where it is either kept or quietly broken.
        let ended = offer(
            &working_ctx(),
            &Scripted::saying(true, &["n"]),
            Some(Condition::Active),
            already_settled(),
        )
        .await;
        assert_eq!(shown(ended), shown(already_settled()));
    }

    #[tokio::test]
    async fn accepting_runs_the_walk_and_reports_what_it_found() {
        // This stack has no media server to prove anything against, so the walk stops —
        // which is the point: the answer comes from the walk rather than from the offer.
        let ended = offer(
            &working_ctx(),
            &Scripted::saying(true, &["y"]),
            Some(Condition::Active),
            already_settled(),
        )
        .await;
        assert_ne!(shown(ended), shown(already_settled()));
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_simply_does_not_offer() {
        // The offer is the last and least important thing setup does; everything above it
        // has already reported on a stack this broken.
        let mut ctx = working_ctx();
        ctx.stack = lemonfiber_core::stack::Source::External(std::path::Path::new("/not-a-stack"));
        let ended = offer(
            &ctx,
            &Scripted::saying(true, &[]),
            Some(Condition::Active),
            already_settled(),
        )
        .await;
        assert_eq!(shown(ended), shown(already_settled()));
    }

    #[test]
    fn silence_takes_the_walk_and_a_clear_no_declines_it() {
        // They are at the end of a setup they chose to run; the walk is what they came for.
        for answer in ["", "y", "yes", "anything"] {
            assert!(yes(&Scripted::saying(true, &[answer])), "{answer}");
        }
        for answer in ["n", "no", "NO", "No"] {
            assert!(!yes(&Scripted::saying(true, &[answer])), "{answer}");
        }
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_is_complained_about_rather_than_reported() {
        // Everything a walk meets is a walk that stopped, which is a report; only
        // the stack itself failing to read is a problem, and a problem is said in
        // the words every other failure is said in.
        let mut ctx = working_ctx();
        ctx.stack = lemonfiber_core::stack::Source::External(std::path::Path::new("/not-a-stack"));
        assert_ne!(shown(walked(&ctx).await), shown(already_settled()));
    }
}
