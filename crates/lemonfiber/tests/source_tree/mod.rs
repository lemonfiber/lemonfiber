//! Every `.rs` file in the workspace, read as text.
//!
//! Shared by the two guards that read it: one asks where a name is allowed to
//! appear, the other asks what the words say. Crawling the tree twice would be two
//! answers to what this workspace contains, and they would drift.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Every `.rs` file in the workspace, keyed by its path relative to the root.
pub(crate) fn sources() -> BTreeMap<PathBuf, String> {
    let root = workspace_root();
    let mut found = BTreeMap::new();
    collect(&root.join("crates"), &root, &mut found);
    assert!(
        found.len() > 10,
        "the crawler found {} files, which means it is looking in the wrong place",
        found.len()
    );
    found
}

pub(crate) fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Some(root) = manifest.parent().and_then(Path::parent) else {
        unreachable!("this crate lives two directories below the workspace root");
    };
    root.to_path_buf()
}

fn collect(dir: &Path, root: &Path, found: &mut BTreeMap<PathBuf, String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, root, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let Ok(text) = fs::read_to_string(&path) else {
                unreachable!("a source file that was just listed should be readable");
            };
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            found.insert(relative, text);
        }
    }
}

/// The half of a file that ships, up to where its own tests begin.
///
/// Found by `mod tests` rather than by the first `#[cfg(test)]`, and the difference
/// is not cosmetic: several files declare a test-only helper module near the top —
/// `render.rs` does it on line 14 — so cutting at the first attribute would discard
/// almost everything those files actually ship. A guard reading that half would
/// report nothing and look as though it had checked.
pub(crate) fn production(text: &str) -> &str {
    let Some(at) = text.lines().position(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("mod tests") || trimmed.starts_with("pub(crate) mod tests")
    }) else {
        return text;
    };
    // One line back from the declaration, which is the blank line above it. The
    // line cap has always counted it this way, and two guards disagreeing about
    // where a file ends is how one of them starts being wrong quietly.
    let kept = at.saturating_sub(1);
    let taken: usize = text.lines().take(kept).map(|line| line.len() + 1).sum();
    text.get(..taken).unwrap_or(text)
}
