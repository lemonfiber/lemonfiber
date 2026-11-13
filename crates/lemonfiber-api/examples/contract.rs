//! Writes the machine-readable contract over the committed directory, its OpenAPI
//! rendering, and what it moved against the previous release's.
//!
//! `just contract` runs it. It writes beside the directory and swaps it in, so a run
//! that fails leaves the committed one as it was. The comparison lives in a test, so
//! a stale directory fails the build rather than this binary.

use std::path::Path;

use lemonfiber_api::contract::openapi::{self, OPENAPI};
use lemonfiber_api::contract::released::{Diff, Snapshot, DIFF, RELEASED_DIR, VERSION};
use lemonfiber_api::contract::{layout, Contract, CONTRACT_DIR};

fn main() {
    let files = match Contract::describe().files() {
        Ok(files) => files,
        Err(faults) => {
            eprintln!(
                "the contract cannot be written as a directory:\n{}",
                faults.join("\n")
            );
            std::process::exit(1);
        }
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let document = match openapi::rendered(&files) {
        Ok(document) => document,
        Err(faults) => {
            eprintln!(
                "the OpenAPI document cannot be rendered:\n{}",
                faults.join("\n")
            );
            std::process::exit(1);
        }
    };
    let dir = root.join(CONTRACT_DIR);
    if let Err(error) = layout::replace(&dir, &files) {
        eprintln!("{}: {error}", dir.display());
        std::process::exit(1);
    }
    let released = root.join(RELEASED_DIR);
    let before = match Snapshot::read(&released) {
        Ok(before) => before,
        Err(error) => {
            eprintln!("{}: {error}", released.display());
            std::process::exit(1);
        }
    };
    let from = std::fs::read_to_string(released.join(VERSION)).unwrap_or_default();
    let diff = Diff::between(from.trim(), &before, &Snapshot::of(&files));
    let diffed = root.join(DIFF);
    if let Err(error) = std::fs::write(&diffed, layout::rendered(&diff)) {
        eprintln!("{}: {error}", diffed.display());
        std::process::exit(1);
    }
    let rendering = root.join(OPENAPI);
    if let Err(error) = std::fs::write(&rendering, layout::rendered(&document)) {
        eprintln!("{}: {error}", rendering.display());
        std::process::exit(1);
    }
}
