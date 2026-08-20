//! Where the stack this repository carries lives on disk.
//!
//! Five test files each resolved it themselves, identically, because a test that drives
//! a real command needs a real stack to drive it against and the path is relative to the
//! crate rather than to the test. One copy, so a stack that moves moves once.

use std::path::{Path, PathBuf};

/// The media stack this repository carries, as an absolute path.
///
/// Resolved once and kept: every caller wants the same directory, and `CARGO_MANIFEST_DIR`
/// is fixed at compile time, so there is nothing to recompute.
pub fn project() -> &'static Path {
    static PROJECT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    PROJECT
        .get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/media-stack"))
}
