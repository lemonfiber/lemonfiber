//! Writes the capability documents over the committed `contract/capabilities/`.
//!
//! `just capability-contracts` runs it. It writes the whole directory afresh, so a capability
//! that was removed leaves no document behind; the comparison lives in a test.

use std::path::Path;

use lemonfiber_contract::documents::{files, DIRECTORY};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(DIRECTORY);
    let _ = std::fs::remove_dir_all(&root);
    for (path, text) in files() {
        let at = root.join(path);
        let written = at
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&at, text));
        if let Err(error) = written {
            eprintln!("{}: {error}", at.display());
            std::process::exit(1);
        }
    }
}
