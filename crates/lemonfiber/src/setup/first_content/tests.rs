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
