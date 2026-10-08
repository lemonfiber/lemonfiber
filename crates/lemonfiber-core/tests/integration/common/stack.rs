//! Where the stack this repository carries lives on disk.
//!
//! Five test files each resolved it themselves, identically, because a test that drives
//! a real command needs a real stack to drive it against and the path is relative to the
//! crate rather than to the test. One copy, so a stack that moves moves once.

use std::path::{Path, PathBuf};

use lemonfiber_core::stack::Source;

/// The media stack this repository carries, as an absolute path.
///
/// Resolved once and kept: every caller wants the same directory, and `CARGO_MANIFEST_DIR`
/// is fixed at compile time, so there is nothing to recompute.
pub fn project() -> &'static Path {
    static PROJECT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    PROJECT
        .get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/media-stack"))
}

/// The same stack, as the source a context reads.
pub fn stack() -> Source {
    Source::External(project())
}

/// The shipped stack, which runs the request gate, written under a scratch directory
/// named for `tag` so a test can write the gate's files beside it.
pub fn with_the_gate(tag: &str) -> std::path::PathBuf {
    let to = lemonfiber_fixtures::scratch::Scratch::named(&format!("gated-{tag}")).kept();
    lemonfiber_fixtures::stack::manifest_into(&to);
    to
}

/// The day the fixture clock is stopped on, as the manifest's date rules read it.
///
/// Read from the same seconds `Stopped::today()` stops on, so a test validating this
/// stack moves forward with the clock rather than going stale on a day of its own. A
/// day that will not convert falls back to the epoch, which every recorded release is
/// after, so a test using it fails rather than passes on a day nobody chose.
pub fn frozen_day() -> lemonfiber_manifest::Date {
    let seconds = i64::try_from(lemonfiber_fixtures::ports::TODAY).unwrap_or_default();
    lemonfiber_manifest::Date::from_unix_seconds(seconds).unwrap_or(lemonfiber_manifest::Date {
        year: 1970,
        month: 1,
        day: 1,
    })
}
