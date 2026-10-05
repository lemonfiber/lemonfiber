use std::cell::Cell;
use std::path::Path;

use super::remembered;
use crate::stack::{Failure, Source};

/// The day the checks are made on, the one the stack's own tests use.
const fn today() -> lemonfiber_manifest::Date {
    lemonfiber_manifest::Date {
        year: 2026,
        month: 10,
        day: 4,
    }
}

/// A stack directory of its own, holding the manifest this repository carries.
fn stack(name: &str) -> &'static Path {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join("stack.toml"),
        include_str!("../../../../../assets/media-stack/stack.toml"),
    );
    Box::leak(dir.into_boxed_path())
}

/// A file that has not changed is checked once, and one written since is checked again.
#[test]
fn a_manifest_is_checked_again_only_once_its_file_has_changed() {
    let at = stack("remembered");
    let source = Source::External(at);
    let today = today();
    let checks = Cell::new(0);
    let checking = || {
        checks.set(checks.get() + 1);
        source.checking(today)
    };

    assert!(remembered(source, today, checking).is_ok());
    assert!(remembered(source, today, checking).is_ok());
    assert_eq!(checks.get(), 1, "the unchanged file was not checked twice");

    let _ = std::fs::write(at.join("stack.toml"), "this is not a manifest");
    let refused = remembered(source, today, checking);
    assert_eq!(checks.get(), 2, "the file written since was checked again");
    assert!(refused.is_err(), "and what it now says is what is answered");
    let _ = std::fs::remove_dir_all(at);
}

/// A refusal is not remembered, so a stack being fixed is read again on the next ask.
#[test]
fn a_refusal_is_worked_out_afresh_each_time() {
    let at = stack("refused");
    let _ = std::fs::write(at.join("stack.toml"), "this is not a manifest");
    let source = Source::External(at);
    let today = today();
    let checks = Cell::new(0);
    let checking = || -> Result<_, Failure> {
        checks.set(checks.get() + 1);
        source.checking(today)
    };

    assert!(remembered(source, today, checking).is_err());
    assert!(remembered(source, today, checking).is_err());
    assert_eq!(checks.get(), 2);
    let _ = std::fs::remove_dir_all(at);
}

/// A stack whose manifest cannot be looked at is checked, and says why it was refused.
#[test]
fn a_stack_with_no_manifest_is_checked_rather_than_remembered() {
    let source = Source::External(Path::new("/lemonfiber/no/such/stack"));
    let today = today();
    assert!(remembered(source, today, || source.checking(today)).is_err());
}
