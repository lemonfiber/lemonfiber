//! Lists the release record's files for the crate to compile in.
//!
//! The record is a directory with a file per release, and a release adds a file. The
//! list is written here rather than kept in the source, so a file added to the
//! directory is carried by the next build without anyone naming it, and none is
//! carried that the directory does not hold.

use std::fmt::Write as _;
use std::path::PathBuf;

/// Where the record is kept, from this crate.
const RECORD: &str = "../../reference/changelog";

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let record = manifest.join(RECORD);
    println!("cargo::rerun-if-changed={}", record.display());

    let mut names: Vec<String> = std::fs::read_dir(&record)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();

    let mut listed = String::from("&[\n");
    for name in &names {
        let path = record.join(name);
        let _ = writeln!(
            listed,
            "    ({name:?}, include_str!({:?})),",
            path.display().to_string()
        );
    }
    listed.push(']');

    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default());
    if let Err(error) = std::fs::write(out.join("carried.rs"), listed) {
        println!("cargo::error=could not list the release record: {error}");
    }
}
