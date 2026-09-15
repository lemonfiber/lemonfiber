//! The package names this workspace refuses, and the two places that refusal is kept.
//!
//! Two families, and they answer different questions — one is about what this
//! product would learn about the person running it, the other about what it could
//! be made to run on their machine. They share a home because they share a
//! mechanism: each is a claim about an absence, each is enforced by `cargo deny`
//! on a dependency bump nobody opened and by a sweep of the resolved graph on a
//! pull request, and each is only as good as those two lists agreeing.
//!
//! They agreed by being copied, once. A second family made that arrangement a
//! choice rather than an accident: `deny.toml` carries one list and the guards
//! carry two, so the equality somebody has to hold is between the file and the
//! union — which no single guard can see from inside its own file.

// Each test binary declaring this module compiles all of it, and each reads the
// part of it that its own subject needs.
#![allow(dead_code)]

use std::collections::BTreeSet;

use crate::source_tree::workspace_root;

/// Package names that would mean this product had started collecting.
///
/// Stems rather than exact names: a family of crates is published under one
/// prefix — an exporter, a core, an integration — and banning the one somebody
/// happens to reach for first would leave the rest. A package matches a stem when
/// its name is the stem, or begins with the stem and a separator.
///
/// This is not a guess at what is out there. It is the list `deny.toml` carries, so
/// the two cannot drift, and [`every_stem`] holds them to each other.
pub(crate) const COLLECTORS: &[&str] = &[
    "amplitude",
    "aptabase",
    "bugsnag",
    "countly",
    "datadog",
    "google-analytics",
    "libhoney",
    "mixpanel",
    "opentelemetry",
    "posthog",
    "rollbar",
    "segment",
    "sentry",
    "snowplow",
];

/// Package names that would give this process a way to run code it was handed.
///
/// Two shapes, and the list is worth reading as two. The first four load a unit of
/// machine code chosen after this binary was built — a shared object, a plugin ABI.
/// The rest evaluate a program supplied as data: a script engine, a bytecode
/// runtime, a JavaScript or WebAssembly host.
///
/// Neither shape is here because it is dangerous in general. They are here because
/// arriving at one is how a plugin stops being data — and a plugin is data is the
/// property the whole extension design rests on, not a habit. The list is the same
/// kind of claim as its sibling above: an absence that is true today by nobody
/// having added anything, and that one commit undoes.
pub(crate) const RUNTIMES: &[&str] = &[
    "abi_stable",
    "dlopen",
    "libloading",
    "sharedlib",
    "boa_engine",
    "deno_core",
    "dyon",
    "extism",
    "gluon",
    "hlua",
    "koto",
    "lua",
    "mlua",
    "quickjs",
    "rhai",
    "rlua",
    "rquickjs",
    "rustpython",
    "starlark",
    "v8",
    "wasm3",
    "wasmedge",
    "wasmer",
    "wasmi",
    "wasmtime",
];

/// A package this workspace is known to depend on, so a reader of `Cargo.lock` that
/// found nothing is told apart from a graph that holds nothing.
const KNOWN_DEPENDENCY: &str = "reqwest";

/// Every stem either family names.
///
/// What `deny.toml` is held to. A stem in one list and not in the file is a stem
/// nothing enforces on the path it was written for, and the file holding a stem
/// neither list names is a refusal with no reason attached to it.
pub(crate) fn every_stem() -> BTreeSet<String> {
    COLLECTORS
        .iter()
        .chain(RUNTIMES)
        .map(|stem| (*stem).to_owned())
        .collect()
}

/// Whether a package name belongs to a family, by its stem.
pub(crate) fn belongs(name: &str, family: &[&str]) -> bool {
    family.iter().any(|stem| {
        name == *stem
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

/// The names cargo-deny is told to refuse.
pub(crate) fn banned() -> BTreeSet<String> {
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
                .filter_map(|entry| entry.get("name").or(Some(entry)))
                .filter_map(toml::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}
