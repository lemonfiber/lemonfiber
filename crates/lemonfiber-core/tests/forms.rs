//! Listing the forms a stack declares and saying what one would start, driven through
//! the dispatcher.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the provider and
//! credential checks are: the app layer is compiled twice, and a path exercised only
//! in-crate has its coverage counted from the copy that never ran.
//!
//! Nothing is faked. A listing reads the stack description and nothing else — no engine,
//! no network, no files of its own — so the real adapters go in and none of them is
//! reached. That is the claim worth making here as much as the listing itself.

mod common;

use common::stack::project;
use std::path::Path;
use std::sync::Arc;

use lemonfiber_core::adapters::{Daemon, Disk, Local, System};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::stack::Source;

fn ctx(stack: Source) -> Ctx {
    Ctx::new(
        Arc::new(Local),
        Arc::new(Daemon::local()),
        Arc::new(System),
        Arc::new(Disk),
        stack,
        Settings::default(),
        Environment::MacOs,
    )
}

/// The stack's own words, and the composability an operator choosing between two forms
/// needs before they try to run both.
#[tokio::test]
async fn the_forms_a_stack_declares_are_listed_as_the_stack_names_them() {
    let listed = dispatch(Command::Forms, &ctx(Source::External(project()))).await;

    assert!(matches!(&listed, Ok(Outcome::Forms(report))
        if report.forms.len() > 1
            && report
                .forms
                .iter()
                .any(|form| form.id == "search" && form.name == "Search" && form.composable)));
}

/// A stack that will not read is the operator's own `--stack-dir`, and they hear about it
/// rather than being handed an empty listing that reads as a stack with nothing in it.
#[tokio::test]
async fn a_stack_that_cannot_be_read_is_a_refusal_rather_than_an_empty_listing() {
    let listed = dispatch(
        Command::Forms,
        &ctx(Source::External(Path::new("/no/such/stack"))),
    )
    .await;

    assert!(listed.is_err());
}

/// The question every start answers for itself before it acts, asked on its own: what
/// does naming this form actually come to?
#[tokio::test]
async fn a_form_says_what_it_would_start_without_starting_anything() {
    let previewed = dispatch(
        Command::Preview {
            forms: vec!["library".to_owned()],
        },
        &ctx(Source::External(project())),
    )
    .await;

    assert!(
        matches!(&previewed, Ok(Outcome::Preview(plan))
            if plan.forms == vec!["library".to_owned()]
                && plan.services.contains(&"jellyfin".to_owned())
                && plan.dropped.is_empty()),
        "the services it holds, in the stack's own words: {previewed:?}"
    );
}

/// Naming a form the stack does not declare is the same refusal wherever it is met, and
/// it arrives here before anything has been started rather than after.
#[tokio::test]
async fn a_preview_of_a_form_that_does_not_exist_is_refused_with_the_one_that_was_meant() {
    let previewed = dispatch(
        Command::Preview {
            forms: vec!["moovies".to_owned()],
        },
        &ctx(Source::External(project())),
    )
    .await;

    let said = previewed.err().map(|problem| {
        (
            problem.summary.clone(),
            problem
                .remedies
                .first()
                .is_some_and(|remedy| remedy.action.starts_with("Did you mean movies?")),
        )
    });
    assert_eq!(
        said,
        Some(("This stack has no form called moovies".to_owned(), true))
    );
}

/// What the configuration leaves out is said with the provider it wanted, because a
/// profile name on its own sends an operator looking for a fault they do not have.
#[tokio::test]
async fn a_preview_names_what_was_left_out_and_what_it_wanted() {
    let usenet_only = Settings {
        protocols: lemonfiber_core::config::Protocols {
            usenet: true,
            torrent: false,
        },
        ..Settings::default()
    };
    let ctx = Ctx::new(
        Arc::new(Local),
        Arc::new(Daemon::local()),
        Arc::new(System),
        Arc::new(Disk),
        Source::External(project()),
        usenet_only,
        Environment::MacOs,
    );

    let previewed = dispatch(
        Command::Preview {
            forms: vec!["dl".to_owned()],
        },
        &ctx,
    )
    .await;

    assert!(
        matches!(&previewed, Ok(Outcome::Preview(plan))
            if plan.dropped.iter().any(|out| out.profile == "torrent"
                && out.needs == lemonfiber_manifest::Protocol::Torrent)),
        "{previewed:?}"
    );
}

/// The same answer, for something that will parse it.
///
/// From here rather than from the surface's own tests: the app layer is compiled a second
/// time for these binaries, and an outcome only ever serialised by the CLI has that copy's
/// arms counted as never run. Asking for the JSON here is also the honest place to assert
/// the contract, since the envelope is the core's promise rather than the terminal's.
#[tokio::test]
async fn a_preview_answers_a_script_under_the_name_it_is_published_by() {
    let previewed = dispatch(
        Command::Preview {
            forms: vec!["library".to_owned()],
        },
        &ctx(Source::External(project())),
    )
    .await;

    let document = previewed
        .ok()
        .and_then(|outcome| outcome.envelope().to_json());
    let said = document.as_deref().map(|json| {
        (
            json.contains(r#""kind":"preview""#),
            json.contains(r#""jellyfin""#),
            json.contains(r#""forms":["library"]"#),
        )
    });
    assert_eq!(
        said,
        Some((true, true, true)),
        "the kind names the question asked, and the payload is the plan itself: {document:?}"
    );
}
