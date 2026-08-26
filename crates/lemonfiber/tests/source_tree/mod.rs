//! Every `.rs` file in the workspace, read as text.
//!
//! Shared by the guards that read it: one asks where a name is allowed to appear,
//! one asks what the words say, and the rest ask what the shipped half reaches.
//! Crawling the tree twice would be two answers to what this workspace contains,
//! and they would drift.
//!
//! [`shipped`] is the narrower corpus those last ones want, and it lives here for
//! the same reason: a second crawl deciding for itself what a release contains
//! would be a second answer to that question too.

// Every test binary that declares this module compiles all of it, and each of them
// reads the tree its own way — the narrower corpus below is what some of them
// want and the raw crawl is what the rest do.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
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

/// The half of every `src/` file that ships, keyed by its path in the workspace.
pub(crate) fn shipped() -> BTreeMap<String, String> {
    let all: BTreeMap<String, String> = sources()
        .into_iter()
        .map(|(path, text)| (path.to_string_lossy().replace('\\', "/"), text))
        .filter(|(path, _)| path.contains("/src/"))
        .collect();
    let unshipped = only_for_tests(&all);
    let found: BTreeMap<String, String> = all
        .iter()
        .filter(|(path, _)| !unshipped.iter().any(|tree| within(path, tree)))
        .map(|(path, text)| (path.clone(), production(text).to_owned()))
        .collect();
    assert!(
        found.len() > 10,
        "the crawl found {} files that ship, which means it is reading the wrong tree",
        found.len()
    );
    assert!(
        !unshipped.is_empty(),
        "no module here is declared test-only, which means this is reading the declaration \
         wrong and is about to hold a fake to a rule meant for what ships"
    );
    found
}

/// The module trees the compiler only builds for tests.
///
/// `production` cuts a file's own tests off the bottom. This is the other shape: a
/// whole module declared behind the test gate, which this workspace uses to keep a
/// fake beside the code it fakes. Nothing in one of those is compiled into a
/// release, so holding one to what a release may do reports a fixture.
fn only_for_tests(all: &BTreeMap<String, String>) -> BTreeSet<String> {
    let mut trees = BTreeSet::new();
    for (path, text) in all {
        let lines: Vec<&str> = text.lines().collect();
        for pair in lines.windows(2) {
            let [gate, declared] = pair else { continue };
            if gate.trim() != "#[cfg(test)]" {
                continue;
            }
            if let Some(name) = module_named(declared) {
                trees.insert(format!("{}/{name}", path.trim_end_matches(".rs")));
            }
        }
    }
    trees
}

/// The module one line declares as a file of its own, where it declares one.
///
/// A `mod tests {` opens a block in the file it is written in and is somebody
/// else's problem; only the form ending in a semicolon names another file.
fn module_named(line: &str) -> Option<&str> {
    let declared = line.trim_start();
    let bare = declared
        .split_once("mod ")
        .filter(|(before, _)| before.is_empty() || before.starts_with("pub"))
        .map(|(_, name)| name)?;
    bare.strip_suffix(';')
}

/// Whether a file belongs to a module tree.
///
/// A directory tree takes the file that declares it as well as the files inside it:
/// `fixtures.rs` and `fixtures/` are one module written in two places.
fn within(path: &str, tree: &str) -> bool {
    path == format!("{tree}.rs") || path.starts_with(&format!("{tree}/"))
}
