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

/// The shipped stack with the request gate added beside Jellyfin, written under a
/// scratch directory named for `tag`.
pub fn with_the_gate(tag: &str) -> std::path::PathBuf {
    let from = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    ));
    let to = lemonfiber_fixtures::scratch::Scratch::named(&format!("gated-{tag}")).kept();
    let _ = std::fs::create_dir_all(&to);
    let read = std::fs::read_to_string(from.join("stack.toml")).unwrap_or_default();
    let jellyfin = read
        .split("[[service]]")
        .find(|block| block.contains("id = \"jellyfin\""))
        .unwrap_or_default();
    let gate = jellyfin
        .replace("id = \"jellyfin\"", "id = \"request-gate\"")
        .replace("name = \"Jellyfin\"", "name = \"Request gate\"")
        .replace("port = 8096", "port = 5057")
        .replace(
            "api = { kind = \"jellyfin\", key_source = \"generated\" }\n",
            "",
        )
        .replace(
            "provides = [\"media.serve\", \"identity.source\"]",
            "provides = []",
        );
    let _ = std::fs::write(to.join("stack.toml"), format!("{read}\n[[service]]{gate}"));
    to
}
