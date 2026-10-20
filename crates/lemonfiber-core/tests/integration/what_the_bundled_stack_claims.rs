//! The embedded stack's claims, judged on every change.
//!
//! A recording that refutes a probe, a claim the contract refuses, and a capability a
//! service provides with no claim fail this; a probe with nothing to judge it against
//! is printed as unproven and passes. The release gate asks the same question of the
//! tree it tags.

use std::path::Path;

use lemonfiber_core::plugin::bundled::Kept;

/// The media stack the bundle keeps its installed parts from.
const MEDIA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/media-stack");

/// The bundle's copy of each part.
const COPIES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/plugins/bundle/plugins"
);

/// Everything the embedded stack provides is claimed, and nothing it claims is refuted by
/// its recording or refused.
#[test]
fn nothing_the_embedded_stack_claims_is_refuted() {
    let manifest = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK)
        .unwrap_or_else(|why| unreachable!("the embedded stack does not read: {why}"));
    let kept = Kept {
        media: Path::new(MEDIA),
        copies: Path::new(COPIES),
    };

    let report = lemonfiber_core::plugin::bundled::judged(&manifest, kept);
    let said = lemonfiber_core::plugin::bundled::said(&report);

    assert!(report.holds(), "{}", said.join("\n"));
    assert!(
        !report.judged.is_empty(),
        "the embedded stack provides nothing, so this judged nothing"
    );
}
