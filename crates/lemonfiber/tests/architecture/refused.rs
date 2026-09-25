//! The package names this workspace refuses, and the two places that refusal is kept.
//!
//! Two families, and they answer different questions — one is about what this
//! product would learn about the person running it, the other about what it could
//! be made to run on their machine. They share a home because they share a
//! mechanism: each is a claim about an absence, each is enforced by `cargo deny`
//! on a dependency bump nobody opened and by a sweep of the resolved graph on a
//! pull request, and each is only as good as those two lists agreeing.
//!
//! `deny.toml` is the one list. Each entry names the family it belongs to in its
//! `reason`, and the guards read the file rather than carrying a copy of it.

use std::collections::BTreeSet;

use crate::source_tree::workspace_root;

/// The `reason` `deny.toml` gives a package that would mean this product had
/// started collecting.
///
/// Stems rather than exact names: a family of crates is published under one
/// prefix — an exporter, a core, an integration — and banning the one somebody
/// happens to reach for first would leave the rest. A package matches a stem when
/// its name is the stem, or begins with the stem and a separator.
pub(crate) const COLLECTING: &str = "reports on the person running it";

/// The `reason` `deny.toml` gives a package that would give this process a way to
/// run code it was handed.
///
/// Two shapes: one loads a unit of machine code chosen after this binary was built
/// — a shared object, a plugin ABI — and the other evaluates a program supplied as
/// data. Arriving at either is how a plugin stops being data, and a plugin is data
/// is the property the whole extension design rests on.
pub(crate) const RUNNING: &str = "runs code it was handed";

/// A package this workspace is known to depend on, so a reader of `Cargo.lock` that
/// found nothing is told apart from a graph that holds nothing.
const KNOWN_DEPENDENCY: &str = "reqwest";

/// The stems `deny.toml` refuses for one reason.
///
/// The file is the only list. A guard that carried its own copy would be a second
/// opinion about what is refused, and the two would drift the first time somebody
/// added to one of them.
pub(crate) fn family(reason: &str) -> Vec<String> {
    banned()
        .into_iter()
        .filter(|(_, given)| given == reason)
        .map(|(stem, _)| stem)
        .collect()
}

/// Whether a package name belongs to a family, by its stem.
pub(crate) fn belongs(name: &str, family: &[String]) -> bool {
    family.iter().any(|stem| {
        name == stem
            || name
                .strip_prefix(stem)
                .is_some_and(|rest| rest.starts_with('-') || rest.starts_with('_'))
    })
}

/// Every package in the resolved dependency graph.
pub(crate) fn resolved() -> BTreeSet<String> {
    let root = workspace_root();
    let Ok(text) = std::fs::read_to_string(root.join("Cargo.lock")) else {
        unreachable!("the workspace this test is compiled from has a lock file")
    };
    let Ok(lock) = text.parse::<toml::Table>() else {
        unreachable!("the lock file cargo writes is TOML")
    };
    let found: BTreeSet<String> = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .map(|packages| {
            packages
                .iter()
                .filter_map(|package| package.get("name"))
                .filter_map(toml::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    assert!(
        found.contains(KNOWN_DEPENDENCY),
        "the graph does not hold {KNOWN_DEPENDENCY}, which means this is reading the wrong \
         file — every claim resting on it would be a claim about nothing"
    );
    found
}

/// The names cargo-deny is told to refuse, each with the reason it gives.
pub(crate) fn banned() -> Vec<(String, String)> {
    let root = workspace_root();
    let Ok(text) = std::fs::read_to_string(root.join("deny.toml")) else {
        unreachable!("the workspace this test is compiled from configures cargo-deny")
    };
    let Ok(config) = text.parse::<toml::Table>() else {
        unreachable!("cargo-deny's configuration is TOML")
    };
    config
        .get("bans")
        .and_then(|bans| bans.get("deny"))
        .and_then(toml::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    let stem = entry.as_str().or_else(|| entry.get("crate")?.as_str())?;
                    let reason = entry.get("reason").and_then(toml::Value::as_str);
                    Some((stem.to_owned(), reason.unwrap_or_default().to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}
