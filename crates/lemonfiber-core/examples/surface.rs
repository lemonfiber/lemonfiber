//! Writes the machine-readable contract's stable surface to stdout.
//!
//! `just surface` redirects it over the committed artefact. It refuses rather than
//! writing where the new surface drops something the committed one describes under
//! an unchanged wire version — which is the whole reason this is a program of its
//! own instead of a redirect: a surface regenerated from the types alone would let
//! whoever removed a field rewrite the record of the field having been there.

use std::path::Path;

use lemonfiber_core::contract::stability::{rendered, Surface, SURFACE_PATH};
use lemonfiber_core::contract::Contract;

fn main() {
    let fresh = Surface::of(&Contract::describe());
    let committed = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(SURFACE_PATH);
    let before = std::fs::read_to_string(committed)
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

    match fresh.to_json() {
        Some(text) => print!("{text}"),
        None => std::process::exit(1),
    }
}
