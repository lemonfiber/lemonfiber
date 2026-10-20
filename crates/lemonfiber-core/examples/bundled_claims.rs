//! Judges every claim the embedded stack makes against the recordings it carries.
//!
//! The release gate runs it, and so does `just bundled-claims`. It exits non-zero where
//! a recording refutes a probe, the contract refuses a claim, or a service provides a
//! capability nothing claims, naming the service, the capability and the probe; a
//! probe nothing could be judged against is listed as unproven and refuses nothing.

use std::path::Path;

use lemonfiber_core::plugin::bundled::Kept;

/// The media stack the bundle keeps its installed parts from, beside the crates.
const MEDIA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/media-stack");

/// The bundle's copy of each part, beside the crates.
const COPIES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/plugins/bundle/plugins"
);

fn main() {
    let manifest = match lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK) {
        Ok(manifest) => manifest,
        Err(why) => {
            eprintln!("the embedded stack could not be read: {why}");
            std::process::exit(1);
        }
    };
    let kept = Kept {
        media: Path::new(MEDIA),
        copies: Path::new(COPIES),
    };
    let report = lemonfiber_core::plugin::bundled::judged(&manifest, kept);
    for said in lemonfiber_core::plugin::bundled::said(&report) {
        println!("{said}");
    }
    if !report.holds() {
        std::process::exit(1);
    }
}
