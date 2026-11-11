//! An adapter reads its upstream's credential and never writes it.
//!
//! lemonfiber writes `upstream.json` into an adapter's configuration directory whenever
//! it creates, rotates or moves that credential, and the keys beside it. The kit every
//! adapter is built on reads them there, and writes one file of its own alone: the
//! cursor it keeps while telling the core what happened, from one module
//! that names none of the files lemonfiber writes.

use std::path::Path;

use crate::source_tree::{production, sources, test_only};

/// The kit every adapter is built on.
const KIT: &str = "crates/lemonfiber-adapter-kit/src/";

/// The one module of the kit that writes: the cursor it keeps.
const CURSOR: &str = "crates/lemonfiber-adapter-kit/src/recounting/kept.rs";

/// Every name of a file lemonfiber writes into an adapter's configuration directory.
const LEMONFIBERS: [&str; 4] = [
    "UPSTREAM_FILE",
    "KEY_FILE",
    "REQUEST_KEY_FILE",
    "EVENTS_KEY_FILE",
];

/// Every spelling of a write to the filesystem.
const WRITES: [&str; 9] = [
    "fs::write(",
    "File::create(",
    "OpenOptions",
    "fs::rename(",
    "fs::copy(",
    "create_dir",
    "set_permissions(",
    "remove_file(",
    "hard_link(",
];

/// The writes `text` spells.
fn writes(text: &str) -> Vec<&'static str> {
    WRITES
        .into_iter()
        .filter(|write| text.contains(write))
        .collect()
}

/// Every production source of the kit, by its path written with `/`.
fn kit() -> Vec<(String, String)> {
    sources()
        .into_iter()
        .filter(|(path, _)| !test_only(path))
        .map(|(path, text)| {
            (
                path.to_string_lossy().replace('\\', "/"),
                production(&text).to_owned(),
            )
        })
        .filter(|(path, _)| path.starts_with(KIT))
        .collect()
}

#[test]
fn the_adapter_kit_writes_its_cursor_alone() {
    let written: Vec<(String, Vec<&str>)> = kit()
        .into_iter()
        .filter(|(path, _)| path != CURSOR)
        .map(|(path, text)| (path, writes(&text)))
        .filter(|(_, found)| !found.is_empty())
        .collect();
    assert!(
        written.is_empty(),
        "an adapter reads upstream.json and never writes it: {written:?}"
    );
}

#[test]
fn the_module_writing_the_cursor_names_no_file_lemonfiber_writes() {
    let cursor = kit().into_iter().find(|(path, _)| path == CURSOR);
    assert!(cursor.is_some(), "{CURSOR} is read");
    let named: Vec<&str> = cursor
        .map(|(_, text)| {
            LEMONFIBERS
                .into_iter()
                .filter(|name| text.contains(name))
                .collect()
        })
        .unwrap_or_default();
    assert!(named.is_empty(), "{CURSOR} names {named:?}");
}

#[test]
fn the_kit_is_read_and_a_write_is_seen() {
    assert!(sources()
        .keys()
        .any(|path| path.starts_with(Path::new(KIT))));
    assert_eq!(writes("std::fs::write(&at, text)"), ["fs::write("]);
    assert!(writes("std::fs::read(&at)").is_empty());
}
