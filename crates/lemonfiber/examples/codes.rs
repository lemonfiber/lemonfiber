//! Writes the error-code reference to stdout.
//!
//! `just codes` redirects it to the committed artefact. The comparison lives in a
//! test, so a stale artefact fails the build rather than this binary.

use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match lemonfiber::codes::render(&root) {
        Ok(text) => print!("{text}"),
        Err(complaints) => {
            for complaint in complaints {
                eprintln!("{complaint}");
            }
            std::process::exit(1);
        }
    }
}
