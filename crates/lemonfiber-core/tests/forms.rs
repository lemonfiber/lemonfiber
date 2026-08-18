//! Listing the forms a stack declares, driven through the dispatcher.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the provider and
//! credential checks are: the app layer is compiled twice, and a path exercised only
//! in-crate has its coverage counted from the copy that never ran.
//!
//! Nothing is faked. A listing reads the stack description and nothing else — no engine,
//! no network, no files of its own — so the real adapters go in and none of them is
//! reached. That is the claim worth making here as much as the listing itself.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_core::adapters::{Daemon, Disk, Local, System};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::stack::Source;

/// The repository's own copy of the stack, so what is listed is what a real installation
/// declares rather than an invented shape.
fn project() -> &'static Path {
    static PROJECT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    PROJECT
        .get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/media-stack"))
}

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
