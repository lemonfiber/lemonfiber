//! What each service is for, asked for through the dispatcher.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the forms listing
//! is: the app layer is compiled twice, and a path exercised only in-crate has its
//! coverage counted from the copy that never ran.
//!
//! Nothing is faked and nothing is reached. This is a read of the stack description and
//! nothing else — no engine, no network, no registry — so the real adapters go in and
//! none of them is touched. That is worth holding here rather than assuming: an answer
//! about what a service is for must be the stack's own answer, and a command that went
//! and asked a service would be reporting what that service says about itself.
//!
//! Two stacks are read. The one this repository carries, which declares nineteen
//! services and has dropped nothing, is what proves the descriptions are real prose
//! rather than a field somebody filled in. A fixture beside it has dropped two services
//! and says so, which is the only way to exercise the record at all — the shipped stack
//! has no removals to read, and a test that only ever saw an empty list would pass
//! against a reader that could not parse one.

use std::path::Path;

use common::stack::project;

use crate::common;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::model::CatalogueReport;
use lemonfiber_core::stack::Source;

fn ctx(stack: Source) -> Ctx {
    lemonfiber_testing::a_live_context().over(stack).build()
}

/// The stack that has dropped something, as an absolute path.
///
/// Leaked because [`Source::External`] holds a `&'static Path`, and resolved from this
/// crate rather than from the working directory, which a test runner does not promise.
fn dropped() -> &'static Path {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dropped");
    Box::leak(path.into_boxed_path())
}

async fn listed(stack: Source) -> CatalogueReport {
    match dispatch(Command::Catalogue, &ctx(stack)).await {
        Ok(Outcome::Catalogue(report)) => report,
        other => unreachable!("the listing answers with itself: {other:?}"),
    }
}

/// The whole point of the listing: nineteen names an operator cannot tell apart, each
/// with a sentence saying what it does for them and a sentence saying what going
/// without costs.
#[tokio::test]
async fn every_service_the_stack_declares_says_what_it_does_for_the_operator() {
    let report = listed(Source::External(project())).await;

    assert!(
        report.services.len() > 1,
        "the whole stack rather than a slice of it: {report:?}"
    );
    let bare: Vec<&str> = report
        .services
        .iter()
        .filter(|service| service.describes.trim().is_empty())
        .map(|service| service.id.as_str())
        .collect();
    assert!(
        bare.is_empty(),
        "a service with nothing said about it is the opacity this listing exists to \
         end: {bare:?}"
    );
    let terse: Vec<&str> = report
        .services
        .iter()
        .filter(|service| service.describes.split_whitespace().count() < 3)
        .map(|service| service.id.as_str())
        .collect();
    assert!(
        terse.is_empty(),
        "these say a word where a sentence was asked for, which is the name again: \
         {terse:?}"
    );
}

/// What its absence costs and how much that matters travel with it, because a
/// description on its own does not tell an operator whether a failure is serious —
/// which is the judgement the whole catalogue is for.
#[tokio::test]
async fn what_going_without_costs_travels_with_what_it_is_for() {
    let report = listed(Source::External(project())).await;

    let silent: Vec<&str> = report
        .services
        .iter()
        .filter(|service| service.without_it.trim().is_empty())
        .map(|service| service.id.as_str())
        .collect();
    assert!(silent.is_empty(), "{silent:?}");
    assert!(
        report
            .services
            .iter()
            .any(|service| service.criticality == lemonfiber_manifest::Criticality::Critical),
        "one service's failure reaches outside this machine, and the listing says which: \
         {report:?}"
    );
}

/// The listing is the stack's own, in the stack's own order — so somebody running a
/// stack of their own is told about theirs, and the order they wrote it in is the order
/// they read it back in.
#[tokio::test]
async fn the_order_is_the_one_the_stack_declares_rather_than_one_of_lemonfibers_own() {
    let report = listed(Source::External(project())).await;
    let manifest = lemonfiber_manifest::Manifest::from_toml(
        &std::fs::read_to_string(project().join("stack.toml")).unwrap_or_default(),
    );

    let declared: Vec<String> = manifest
        .map(|manifest| {
            manifest
                .services
                .iter()
                .map(|service| service.id.clone())
                .collect()
        })
        .unwrap_or_default();
    let listed: Vec<String> = report
        .services
        .iter()
        .map(|service| service.id.clone())
        .collect();
    assert!(
        !declared.is_empty(),
        "the manifest was read, so two empty lists cannot pass this"
    );
    assert_eq!(listed, declared);
}

/// A stack that has dropped something says what went, which of its own versions it went
/// in, why, and what took its place — and says nothing took its place where nothing did,
/// which is the answer a record has to be able to give without inventing a successor.
#[tokio::test]
async fn a_service_that_went_is_recorded_with_its_reason_and_its_replacement() {
    let report = listed(Source::External(dropped())).await;

    let recorded: Vec<(&str, &str, bool, Option<&str>)> = report
        .removed
        .iter()
        .map(|gone| {
            (
                gone.id.as_str(),
                gone.removed_in.as_str(),
                gone.reason.split_whitespace().count() > 3,
                gone.replaced_by.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        recorded,
        vec![
            ("readarr", "0.1.0", true, Some("bindery")),
            ("booksonic", "0.2.0", true, None),
        ]
    );
}

/// What the shipped stack records is what the shipped stack records, however much that
/// is — asserted against the manifest rather than against a number.
///
/// This began as *the stack this repository carries has dropped nothing*, which was
/// true and is the shape of claim that goes stale without anybody touching it: the
/// stack is a submodule and it is about to start recording removals, so a count
/// written here would fail the day its pin moved, for a reason that is nobody's
/// mistake. Read off the manifest instead, and it holds either way.
#[tokio::test]
async fn what_the_stack_records_as_gone_is_what_the_listing_hands_back() {
    let report = listed(Source::External(project())).await;
    let manifest = lemonfiber_manifest::Manifest::from_toml(
        &std::fs::read_to_string(project().join("stack.toml")).unwrap_or_default(),
    );

    let declared: Vec<String> = manifest
        .map(|manifest| {
            manifest
                .removed
                .iter()
                .map(|gone| gone.id.clone())
                .collect()
        })
        .unwrap_or_default();
    let listed: Vec<String> = report.removed.iter().map(|gone| gone.id.clone()).collect();

    assert_eq!(listed, declared);
}

/// A stack that will not read is the operator's own `--stack-dir`, and they hear about
/// it rather than being handed an empty listing that reads as a stack holding nothing.
#[tokio::test]
async fn a_stack_that_cannot_be_read_is_a_refusal_rather_than_an_empty_listing() {
    let answered = dispatch(
        Command::Catalogue,
        &ctx(Source::External(Path::new("/no/such/stack"))),
    )
    .await;

    assert!(answered.is_err());
}

/// The same answer, for something that will parse it.
///
/// From here rather than from the surface's own tests: the app layer is compiled a
/// second time for these binaries, and an outcome only ever serialised by the CLI has
/// that copy's arms counted as never run.
#[tokio::test]
async fn it_answers_a_script_under_the_name_it_is_published_by() {
    let answered = dispatch(Command::Catalogue, &ctx(Source::External(dropped()))).await;

    let document = answered
        .ok()
        .and_then(|outcome| outcome.envelope().to_json());
    let said = document.as_deref().map(|json| {
        (
            json.contains(r#""kind":"catalogue""#),
            json.contains(r#""describes":"Watches for the books"#),
            json.contains(r#""replaced_by":"bindery""#),
            json.contains(r#""replaced_by":null"#),
        )
    });
    assert_eq!(
        said,
        Some((true, true, true, true)),
        "the kind names the question asked, and the payload is the record itself: \
         {document:?}"
    );
}
