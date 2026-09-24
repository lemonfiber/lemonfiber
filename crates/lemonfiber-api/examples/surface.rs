//! Writes the machine-readable contract's stable surface over the committed artefact.
//!
//! `just surface` runs it, and it writes beside the artefact and renames over it, so
//! a run that fails leaves the committed file as it was. It refuses rather than
//! writing where the new surface drops something the committed one describes under
//! an unchanged wire version — which is the whole reason this is a program of its
//! own instead of a redirect: a surface regenerated from the types alone would let
//! whoever removed a field rewrite the record of the field having been there.

use std::path::Path;

use lemonfiber_api::contract::stability::{rendered, Surface, SURFACE_PATH};
use lemonfiber_api::contract::Contract;

fn main() {
    let fresh = Surface::of(&Contract::describe());
    let committed = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(SURFACE_PATH);
    let before = std::fs::read_to_string(&committed)
        .ok()
        .as_deref()
        .and_then(Surface::parse)
        .unwrap_or_default();

    let broken = Surface::broken(&before, &fresh);
    if !broken.is_empty() {
        eprintln!(
            "this surface drops what the committed one describes, under an unchanged \
             api_version:\n{}\nEither put them back, or increment API_VERSION first.",
            rendered(&broken)
        );
        std::process::exit(1);
    }

    let Some(text) = fresh.to_json() else {
        std::process::exit(1);
    };
    let next = committed.with_extension("json.next");
    if let Err(error) =
        std::fs::write(&next, text).and_then(|()| std::fs::rename(&next, &committed))
    {
        eprintln!("{}: {error}", committed.display());
        std::process::exit(1);
    }
}
