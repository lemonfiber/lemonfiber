//! An adapter reads its upstream's credential and never writes it.
//!
//! lemonfiber writes `upstream.json` into an adapter's configuration directory whenever
//! it creates, rotates or moves that credential. The kit every adapter is built on reads
//! it there, and writes no file at all.

use std::path::Path;

use crate::source_tree::{production, sources, test_only};

/// The kit every adapter is built on.
const KIT: &str = "crates/lemonfiber-adapter-kit/src/";

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

#[test]
fn the_adapter_kit_writes_no_file() {
    let written: Vec<(String, Vec<&str>)> = sources()
        .iter()
        .filter(|(path, _)| {
            path.to_string_lossy().replace('\\', "/").starts_with(KIT) && !test_only(path)
        })
        .map(|(path, text)| (path.display().to_string(), writes(production(text))))
        .filter(|(_, found)| !found.is_empty())
        .collect();
    assert!(
        written.is_empty(),
        "an adapter reads upstream.json and never writes it: {written:?}"
    );
}

#[test]
fn the_kit_is_read_and_a_write_is_seen() {
    assert!(sources()
        .keys()
        .any(|path| path.starts_with(Path::new(KIT))));
    assert_eq!(writes("std::fs::write(&at, text)"), ["fs::write("]);
    assert!(writes("std::fs::read(&at)").is_empty());
}
