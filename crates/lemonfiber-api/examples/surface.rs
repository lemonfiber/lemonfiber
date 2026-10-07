//! Writes the machine-readable contract's stable surface over the committed directory.
//!
//! `just surface` runs it, and it writes beside the directory and swaps it in, so a
//! run that fails leaves the committed one as it was. It refuses rather than
//! writing where the new surface drops something the committed one describes under
//! an unchanged wire version and no declaration accepts it — which is the whole
//! reason this is a program of its own instead of a redirect: a surface regenerated
//! from the types alone would let whoever removed a field rewrite the record of the
//! field having been there.

use std::path::Path;

use lemonfiber_api::contract::stability::{rendered, Surface, SURFACE_DIR};
use lemonfiber_api::contract::{layout, Contract};

fn main() {
    let fresh = Surface::of(&Contract::describe());
    let committed = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(SURFACE_DIR);
    let before = layout::read(&committed)
        .ok()
        .as_ref()
        .and_then(Surface::from_files)
        .unwrap_or_default();

    let broken = Surface::refused(&before, &fresh);
    if !broken.is_empty() {
        eprintln!(
            "this surface drops what the committed one describes, under an unchanged \
             api_version:\n{}\nEither put them back, declare each in \
             stability::DECLARED, or increment API_VERSION first.",
            rendered(&broken)
        );
        std::process::exit(1);
    }

    let Some(files) = fresh.files() else {
        std::process::exit(1);
    };
    if let Err(error) = layout::replace(&committed, &files) {
        eprintln!("{}: {error}", committed.display());
        std::process::exit(1);
    }
}
