//! Which platform this is, and what differs because of it, are decided in one crate.
//!
//! `lemonfiber-platform` holds the operating system, the environment resolved from it,
//! and every operation whose meaning differs from one platform to the next: a private
//! directory, an owner-only file, an open that refuses a link and refuses to wait, a
//! change time, an owner, two readings of one file, a mode, a directory flushed, and the
//! walk beneath a directory through each step's own handle. Everything else calls it.
//! Conditional compilation scattered through call sites is how a codebase becomes
//! impossible to exercise on any single machine, and how a second, slightly different
//! answer to one question appears beside the first.
//!
//! A test is the one place a platform may still be asked, and only for what a test has
//! to plant or read that some platforms do not have. [`A_TEST_MAY_ASK`] names each
//! question and why; anything else a test asks is refused like shipped code.

use std::collections::BTreeSet;
use std::fs;

use syn::visit::Visit;

use crate::source_tree::{parsed, test_only, workspace_root};

/// The crate that may ask.
const PLATFORM: &str = "crates/lemonfiber-platform/";

/// The names a `cfg` asks which platform this is with.
const ASKING: &[&str] = &["unix", "windows", "target_os", "target_family"];

/// What a test may still ask of the platform, and what it needs the answer for.
const A_TEST_MAY_ASK: &[(&str, &str)] = &[
    (
        "unix",
        "a link, a named pipe, a file mode, an owner or a Unix socket, which a test plants \
         to prove it is refused or reads to prove what was made",
    ),
    (
        "target_os = \"linux\"",
        "a file name that is not UTF-8, which macOS refuses to make",
    ),
];

/// Every platform question a file asks, in a `cfg`, a `cfg_attr` or a `cfg!`, spelled
/// as the predicate: `unix`, `windows`, or `target_os = "…"`.
#[derive(Default)]
struct Asked(BTreeSet<String>);

impl Asked {
    fn of(file: &syn::File) -> Self {
        let mut asked = Self::default();
        asked.visit_file(file);
        asked
    }

    fn read(&mut self, tokens: &str) {
        let words: Vec<&str> = tokens
            .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '"'))
            .filter(|word| !word.is_empty())
            .collect();
        for (at, word) in words.iter().enumerate() {
            if !ASKING.contains(word) {
                continue;
            }
            match words.get(at + 1) {
                Some(value) if value.starts_with('"') => {
                    self.0.insert(format!("{word} = {value}"));
                }
                _ => {
                    self.0.insert((*word).to_owned());
                }
            }
        }
    }
}

impl<'ast> Visit<'ast> for Asked {
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        let path = attribute.path();
        if !(path.is_ident("cfg") || path.is_ident("cfg_attr")) {
            return;
        }
        if let Ok(list) = attribute.meta.require_list() {
            self.read(&list.tokens.to_string());
        }
    }

    fn visit_macro(&mut self, called: &'ast syn::Macro) {
        if called.path.is_ident("cfg") {
            self.read(&called.tokens.to_string());
        }
        syn::visit::visit_macro(self, called);
    }
}

/// Every file outside the platform crate that asks, with what it asks.
fn asking() -> Vec<(String, BTreeSet<String>)> {
    let read = parsed("crates");
    assert!(
        read.iter()
            .any(|(path, file)| path.starts_with(PLATFORM) && !Asked::of(file).0.is_empty()),
        "nothing in {PLATFORM} asks which platform this is, so this is reading the wrong tree"
    );
    read.into_iter()
        .filter(|(path, _)| !path.starts_with(PLATFORM))
        .map(|(path, file)| {
            let shown = path.to_string_lossy().replace('\\', "/");
            (path, shown, Asked::of(&file).0)
        })
        .filter(|(_, _, asked)| !asked.is_empty())
        .map(|(path, shown, asked)| {
            let kept = if test_only(&path) {
                asked
                    .into_iter()
                    .filter(|one| !A_TEST_MAY_ASK.iter().any(|(may, _)| may == one))
                    .collect()
            } else {
                asked
            };
            (shown, kept)
        })
        .filter(|(_, asked)| !asked.is_empty())
        .collect()
}

/// Nothing outside `lemonfiber-platform` asks which platform this is, but a test asking
/// what [`A_TEST_MAY_ASK`] names.
#[test]
fn only_the_platform_crate_asks_which_platform_this_is() {
    let asking = asking();
    assert!(
        asking.is_empty(),
        "these ask which platform this is — call `lemonfiber-platform` instead, and put \
         what differs there: {asking:?}"
    );
}

/// Every question a test may ask is one some test still asks.
///
/// The direction that goes unnoticed: an exemption nothing uses is room for the next
/// file to ask without anybody deciding it should.
#[test]
fn every_question_a_test_may_ask_is_still_asked() {
    let mut asked = BTreeSet::new();
    for (path, file) in parsed("crates") {
        if test_only(&path) && !path.starts_with(PLATFORM) {
            asked.extend(Asked::of(&file).0);
        }
    }
    for (may, why) in A_TEST_MAY_ASK {
        assert!(
            asked.contains(*may),
            "no test asks `{may}` any more — it was allowed for {why}, so take it off"
        );
    }
}

/// No crate but `lemonfiber-platform` takes a dependency only on some platforms, and
/// it depends on nothing of ours, so every crate may reach it.
#[test]
fn only_the_platform_crate_depends_on_the_platform() {
    let root = workspace_root();
    let Ok(crates) = fs::read_dir(root.join("crates")) else {
        unreachable!("the workspace keeps its crates under crates/");
    };
    let mut read = 0;
    for entry in crates.flatten() {
        let Ok(manifest) = fs::read_to_string(entry.path().join("Cargo.toml")) else {
            continue;
        };
        let Ok(table) = manifest.parse::<toml::Table>() else {
            unreachable!("every crate manifest in the workspace parses");
        };
        read += 1;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "lemonfiber-platform" {
            let ours: Vec<&String> = declared(&table)
                .into_iter()
                .filter(|dependency| dependency.starts_with("lemonfiber"))
                .collect();
            assert!(
                ours.is_empty(),
                "lemonfiber-platform depends on {ours:?} — it must depend on nothing of \
                 ours, or the crates it serves could not all reach it"
            );
        } else {
            assert!(
                !table.contains_key("target"),
                "{name} takes a dependency on some platforms only — what differs by \
                 platform belongs in lemonfiber-platform"
            );
        }
    }
    assert!(
        read > 10,
        "{read} crate manifests were read, which means this is reading the wrong tree"
    );
}

/// The names of every dependency a manifest declares to build with, on every platform
/// and on some.
fn declared(manifest: &toml::Table) -> Vec<&String> {
    let kinds = ["dependencies", "build-dependencies"];
    let targets = manifest
        .get("target")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::Table::values)
        .filter_map(toml::Value::as_table);
    std::iter::once(manifest)
        .chain(targets)
        .flat_map(|table| kinds.iter().filter_map(|kind| table.get(*kind)))
        .filter_map(toml::Value::as_table)
        .flat_map(toml::Table::keys)
        .collect()
}
