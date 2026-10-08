//! Writes the machine-readable contract over the committed directory.
//!
//! `just contract` runs it. It writes beside the directory and swaps it in, so a run
//! that fails leaves the committed one as it was. The comparison lives in a test, so
//! a stale directory fails the build rather than this binary.

use std::path::Path;

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
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(CONTRACT_DIR);
    if let Err(error) = layout::replace(&dir, &files) {
        eprintln!("{}: {error}", dir.display());
        std::process::exit(1);
    }
}
