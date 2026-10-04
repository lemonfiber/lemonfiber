//! Writes the set of adapters a plugin's service may name to stdout.
//!
//! `just adapters` redirects it to the committed artefact.

fn main() {
    if let Some(text) = lemonfiber_core::plugin::adapters() {
        print!("{text}");
    }
}
