//! Each deferred check declares the family and budget of the check it builds.

use super::{providing, stored, tunnel, wired, Stack};
use crate::doctor::deferred::Deferred;
use crate::doctor::Check;
use crate::test_support::a_context;

/// Whether `deferred`, once run, built a check of the family and budget it declared
/// before it was built.
async fn declared_what_it_built(deferred: &Deferred) -> bool {
    let _findings = deferred.run().await;
    deferred.built().is_some_and(|built| {
        built.category() == deferred.category() && built.budget() == deferred.budget()
    })
}

/// Narrowing reads the family, and the run is held to the budget, before the check
/// exists — so each has to be the one the built check reports, or a check would be
/// narrowed out of its own family or cut short of its own budget.
#[tokio::test]
async fn each_declares_the_family_and_budget_of_the_check_it_builds() {
    let ctx = a_context().build();
    // Collected rather than matched: the stack this repo embeds always parses, so a
    // fallback would be a branch no passing test can reach.
    let stacks: Vec<Stack> = ctx
        .stack
        .checked_manifest(ctx.today())
        .into_iter()
        .map(|manifest| Stack {
            manifest,
            installed: Vec::new(),
        })
        .collect();
    for stack in &stacks {
        let manifest = &stack.manifest;
        for deferred in [
            stored(&ctx, manifest, None),
            tunnel(&ctx, manifest, None, false),
            tunnel(&ctx, manifest, None, true),
            providing(&ctx, &manifest.services, None),
            wired(&ctx, stack, None),
        ] {
            assert!(
                declared_what_it_built(&deferred).await,
                "{:?} declared otherwise",
                deferred.category()
            );
        }
    }
    assert_eq!(stacks.len(), 1, "the embedded stack parses");
}
