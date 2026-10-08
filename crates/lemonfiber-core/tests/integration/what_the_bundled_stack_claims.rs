//! The embedded stack's claims, judged on every change.
//!
//! A recording that refutes a probe, a claim the contract refuses, and a capability a
//! service provides with no claim fail this; a probe with nothing to judge it against
//! is printed as unproven and passes. The release gate asks the same question of the
//! tree it tags.

use std::path::Path;

/// The stack this build embeds.
const STACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/media-stack");

/// Everything the embedded stack provides is claimed, and nothing it claims is refuted by
/// its recording or refused.
#[test]
fn nothing_the_embedded_stack_claims_is_refuted() {
    let stack = Path::new(STACK);
    let text = lemonfiber_manifest::read(stack).unwrap_or_default();
    let manifest = lemonfiber_manifest::Manifest::from_toml(&text)
        .unwrap_or_else(|why| unreachable!("the embedded stack does not read: {why}"));

    let report = lemonfiber_core::plugin::bundled::judged(&manifest, stack);
    let said = lemonfiber_core::plugin::bundled::said(&report);

    assert!(report.holds(), "{}", said.join("\n"));
    assert!(
        !report.judged.is_empty(),
        "the embedded stack provides nothing, so this judged nothing"
    );
}
