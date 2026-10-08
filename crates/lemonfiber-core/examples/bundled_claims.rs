//! Judges every claim the embedded stack makes against the recordings it carries.
//!
//! The release gate runs it, and so does `just bundled-claims`. It exits non-zero where
//! a recording refutes a probe, the contract refuses a claim, or a service provides a
//! capability nothing claims, naming the service, the capability and the probe; a
//! probe nothing could be judged against is listed as unproven and refuses nothing.

use std::path::Path;

/// The stack this build embeds, beside the crates.
const STACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/media-stack");

fn main() {
    let stack = Path::new(STACK);
    let manifest = match lemonfiber_manifest::read(stack)
        .and_then(|text| lemonfiber_manifest::Manifest::from_toml(&text))
    {
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
