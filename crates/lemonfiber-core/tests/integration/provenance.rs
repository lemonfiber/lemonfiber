//! Where the services come from, asked for through the dispatcher.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the forms listing
//! is: the app layer is compiled twice, and a path exercised only in-crate has its
//! coverage counted from the copy that never ran.
//!
//! Nothing is faked and nothing is reached. This is a read of the stack description and
//! nothing else — no engine, no network, no registry — so the real adapters go in and
//! none of them is touched. That matters here more than in most of these: an answer
//! about what is bundled must be an answer about what this build carries, and a command
//! that went and asked a registry would be reporting what is published today rather
//! than what is pinned.

use std::path::Path;

use common::stack::project;

use crate::common;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::model::ProvenanceReport;
use lemonfiber_core::stack::Source;

fn ctx(stack: Source) -> Ctx {
    lemonfiber_testing::a_live_context().over(stack).build()
}

async fn listed() -> ProvenanceReport {
    match dispatch(Command::Provenance, &ctx(Source::External(project()))).await {
        Ok(Outcome::Provenance(report)) => report,
        other => unreachable!("the listing answers with itself: {other:?}"),
    }
}

/// The claim this stack makes about itself, service by service, in a form somebody can
/// go and check: an identifier, a project, and the exact image the identifier is being
/// claimed about.
#[tokio::test]
async fn every_service_the_stack_declares_says_what_it_is_under_and_where_it_is_from() {
    let report = listed().await;

    assert!(
        report.services.len() > 1,
        "the whole stack rather than a slice of it: {report:?}"
    );
    let bare: Vec<&str> = report
        .services
        .iter()
        .filter(|service| {
            service.license.is_empty() || service.upstream.is_empty() || service.pinned.is_empty()
        })
        .map(|service| service.id.as_str())
        .collect();
    assert!(
        bare.is_empty(),
        "a service with nothing recorded is a service nobody can check: {bare:?}"
    );
}

/// The pin is the exact tag rather than something that moves under it, which is what
/// makes the rest of the entry worth reading: a licence checked against a project says
/// nothing about a version nobody can name.
#[tokio::test]
async fn the_pin_is_an_exact_version_and_the_image_it_belongs_to() {
    let report = listed().await;

    let sonarr = report
        .services
        .iter()
        .find(|service| service.id == "sonarr")
        .cloned();
    let said = sonarr.map(|service| {
        (
            service.image.contains('/'),
            service.pinned.is_empty(),
            service.pinned.contains("latest"),
        )
    });
    assert_eq!(
        said,
        Some((true, false, false)),
        "an image reference and a tag that names one version: {report:?}"
    );
}

/// The listing is the stack's own, in the stack's own order — so somebody running a
/// stack of their own is told about theirs, and the order they wrote it in is the order
/// they read it back in.
#[tokio::test]
async fn the_order_is_the_one_the_stack_declares_rather_than_one_of_lemonfibers_own() {
    let report = listed().await;
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

/// A stack that will not read is the operator's own `--stack-dir`, and they hear about
/// it rather than being handed an empty listing that reads as a stack bundling nothing.
#[tokio::test]
async fn a_stack_that_cannot_be_read_is_a_refusal_rather_than_an_empty_listing() {
    let answered = dispatch(
        Command::Provenance,
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
    let answered = dispatch(Command::Provenance, &ctx(Source::External(project()))).await;

    let document = answered
        .ok()
        .and_then(|outcome| outcome.envelope().to_json());
    let said = document.as_deref().map(|json| {
        (
            json.contains(r#""kind":"provenance""#),
            json.contains(r#""license":"GPL-3.0-only""#),
            json.contains(r#""upstream":"https://github.com/Sonarr/Sonarr""#),
        )
    });
    assert_eq!(
        said,
        Some((true, true, true)),
        "the kind names the question asked, and the payload is the record itself: \
         {document:?}"
    );
}
