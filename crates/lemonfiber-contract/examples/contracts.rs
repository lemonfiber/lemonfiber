//! Writes the capability documents over the committed `contract/capabilities/`.
//!
//! `just capability-contracts` runs it. It writes the whole directory afresh but for the index
//! the last release published, so a capability
//! that was removed leaves no document behind; the comparison lives in a test.

use std::path::Path;

use lemonfiber_contract::documents::{files, DIRECTORY};
use lemonfiber_contract::released::RELEASED;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(DIRECTORY);
    for entry in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        } else if entry.file_name() != RELEASED {
            let _ = std::fs::remove_file(&path);
        }
    }
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
