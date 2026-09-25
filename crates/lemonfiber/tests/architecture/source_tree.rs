//! Every `.rs` file in the workspace, read once, as text and as syntax.
//!
//! Shared by every check in this binary: one asks where a name is allowed to appear,
//! one asks what the words say, and the rest ask what the shipped half reaches or
//! how the code is shaped. The crawl is made once for the process, and the checks
//! that ask about the shape of the code parse from it, so every check reads the same
//! answer to what the workspace holds.
//!
//! [`shipped`] is the narrower corpus the guards about a release want, and it lives
//! here for the same reason: a second crawl deciding for itself what a release
//! contains would be a second answer to that question too.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Every `.rs` file in the workspace, keyed by its path relative to the root.
pub(crate) fn sources() -> BTreeMap<PathBuf, String> {
    crawled().clone()
}

/// The crawl, made the first time anything asks for it.
fn crawled() -> &'static BTreeMap<PathBuf, String> {
    static CRAWLED: OnceLock<BTreeMap<PathBuf, String>> = OnceLock::new();
    CRAWLED.get_or_init(|| {
        let root = workspace_root();
        let mut found = BTreeMap::new();
        collect(&root.join("crates"), &root, &mut found);
        assert!(
            found.len() > 10,
            "the crawler found {} files, which means it is looking in the wrong place",
            found.len()
        );
        found
    })
}

/// Every `.rs` file under the named directory, as the syntax tree the compiler reads.
///
/// Parsed on each call rather than kept: a syntax tree is not shared across threads,
/// and the checks that want one each ask about a small part of the tree. The files
/// that are fixtures of deliberate violations do not end in `.rs`, so everything the
/// crawl finds is Rust, and a file that does not parse is a fault in the tree.
pub(crate) fn parsed(under: &str) -> Vec<(PathBuf, syn::File)> {
    crawled()
        .iter()
        .filter(|(path, _)| path.starts_with(under))
        .map(|(path, text)| {
            let Ok(file) = syn::parse_file(text) else {
                unreachable!("{} is Rust the compiler builds", path.display());
            };
            (path.clone(), file)
        })
        .collect()
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

/// Whether a file is built only for tests: outside `src/`, or inside a module tree
/// declared behind `#[cfg(test)]` — which is where every file's own tests live.
///
/// The per-file guards read `production` of each file they are handed, which cuts a
/// file's tests off its bottom; a file that is nothing but tests has no bottom to cut,
/// so it is refused here instead.
pub(crate) fn test_only(path: &Path) -> bool {
    let path = path.to_string_lossy().replace('\\', "/");
    !path.contains("/src/") || unshipped().iter().any(|tree| within(&path, tree))
}

/// The test-only module trees of every `src/` file, found the first time anything asks.
fn unshipped() -> &'static BTreeSet<String> {
    static UNSHIPPED: OnceLock<BTreeSet<String>> = OnceLock::new();
    UNSHIPPED.get_or_init(|| {
        let all: BTreeMap<String, String> = crawled()
            .iter()
            .map(|(path, text)| (path.to_string_lossy().replace('\\', "/"), text.clone()))
            .filter(|(path, _)| path.contains("/src/"))
            .collect();
        only_for_tests(&all)
    })
}

/// A file's text with every reach through `ctx.seams.` written as the `ctx.` reach
/// it is.
///
/// A context carries its ports in one `seams` field, so `ctx.seams.http` is how the
/// transport is reached. The guards that name what a family may reach name the seam,
/// and read through this so a chain broken across lines still names it.
pub(crate) fn unseamed(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("ctx.seams") {
        let (before, after) = rest.split_at(at);
        out.push_str(before);
        out.push_str("ctx.");
        let after = after.get("ctx.seams".len()..).unwrap_or_default();
        rest = after.trim_start().strip_prefix('.').unwrap_or(after);
    }
    out.push_str(rest);
    out
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

/// The workspace crates a release is built from: the binary's own and every one it
/// depends on, followed through each of theirs.
///
/// A crate only a `[dev-dependencies]` table names is built for tests and nothing
/// else, so a check about what a release does asks this before holding a file to it.
pub(crate) fn crates_that_ship() -> BTreeSet<String> {
    let root = workspace_root();
    let mut ships = BTreeSet::new();
    let mut open = vec!["lemonfiber".to_owned()];
    while let Some(name) = open.pop() {
        let Ok(manifest) = fs::read_to_string(root.join("crates").join(&name).join("Cargo.toml"))
        else {
            continue;
        };
        if !ships.insert(name) {
            continue;
        }
        let Ok(read) = manifest.parse::<toml::Table>() else {
            unreachable!("every crate manifest in the workspace parses");
        };
        let targets = read
            .get("target")
            .and_then(toml::Value::as_table)
            .into_iter()
            .flat_map(toml::Table::values)
            .filter_map(toml::Value::as_table);
        let tables = std::iter::once(&read)
            .chain(targets)
            .filter_map(|table| table.get("dependencies"))
            .filter_map(toml::Value::as_table);
        open.extend(tables.flat_map(toml::Table::keys).cloned());
    }
    assert!(
        ships.len() > 2,
        "the binary was found to be built from {ships:?}, which means this is reading the \
         wrong manifests"
    );
    ships
}

/// Whether the file at this workspace path is built into a release.
pub(crate) fn in_a_crate_that_ships(path: &str, ships: &BTreeSet<String>) -> bool {
    path.strip_prefix("crates/")
        .and_then(|rest| rest.split('/').next())
        .is_some_and(|name| ships.contains(name))
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
                trees.insert(format!("{}/{name}", directory_of(path)));
            }
        }
    }
    trees
}

/// The directory a file's own child modules live in: beside a crate root or a
/// `mod.rs`, and in a directory named for the file otherwise.
fn directory_of(path: &str) -> &str {
    let stem = path.trim_end_matches(".rs");
    for root in ["/lib", "/main", "/mod"] {
        if let Some(parent) = stem.strip_suffix(root) {
            return parent;
        }
    }
    stem
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
