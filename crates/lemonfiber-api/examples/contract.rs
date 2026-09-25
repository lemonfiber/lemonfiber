//! Writes the machine-readable contract over the committed artefact.
//!
//! `just contract` runs it. It writes beside the artefact and renames over it, so a
//! run that fails leaves the committed file as it was. The comparison lives in a
//! test, so a stale artefact fails the build rather than this binary.

use std::path::Path;

use lemonfiber_api::contract::{Contract, CONTRACT_PATH};

fn main() {
    let Some(text) = Contract::describe().to_json() else {
        std::process::exit(1);
    };
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(CONTRACT_PATH);
    let next = path.with_extension("json.next");
    if let Err(error) = std::fs::write(&next, text).and_then(|()| std::fs::rename(&next, &path)) {
        eprintln!("{}: {error}", path.display());
        std::process::exit(1);
    }
}
