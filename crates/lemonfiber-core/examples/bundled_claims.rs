//! Judges every claim the embedded stack makes against the recordings it carries.
//!
//! The release gate runs it, and so does `just bundled-claims`. It exits non-zero where
//! a recording refutes a probe or the contract refuses a claim, naming the service, the
//! capability and the probe; a probe nothing could be judged against, and a capability
//! nothing claims, are listed as unproven and refuse nothing.

use std::path::Path;

/// The stack this build embeds, beside the crates.
const STACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/media-stack");

fn main() {
    let stack = Path::new(STACK);
    let manifest = match std::fs::read_to_string(stack.join("stack.toml"))
        .map_err(|unreadable| unreadable.to_string())
        .and_then(|text| {
            lemonfiber_manifest::Manifest::from_toml(&text).map_err(|why| why.to_string())
        }) {
        Ok(manifest) => manifest,
        Err(why) => {
            eprintln!("the embedded stack could not be read: {why}");
            std::process::exit(1);
        }
    };
    let report = lemonfiber_core::plugin::bundled::judged(&manifest, stack);
    for said in lemonfiber_core::plugin::bundled::said(&report) {
        println!("{said}");
    }
    if !report.holds() {
        std::process::exit(1);
    }
}
