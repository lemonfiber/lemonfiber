//! The embedded stack's claims, judged on every change.
//!
//! A recording that refutes a probe, and a claim the contract refuses, fail this; a
//! probe with nothing to judge it against and a capability nothing claims are printed
//! as unproven and pass. The release gate asks the same question of the tree it tags.

use std::path::Path;

/// The stack this build embeds.
const STACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/media-stack");

/// Nothing the embedded stack claims is refuted by its recording or refused.
#[test]
fn nothing_the_embedded_stack_claims_is_refuted() {
    let stack = Path::new(STACK);
    let text = std::fs::read_to_string(stack.join("stack.toml")).unwrap_or_default();
    let manifest = lemonfiber_manifest::Manifest::from_toml(&text)
        .unwrap_or_else(|why| unreachable!("the embedded stack does not read: {why}"));

    let report = lemonfiber_core::plugin::bundled::judged(&manifest, stack);
    let said = lemonfiber_core::plugin::bundled::said(&report);

    assert!(report.holds(), "{}", said.join("\n"));
    assert!(
        !report.unclaimed.is_empty() || !report.judged.is_empty(),
        "the embedded stack provides nothing, so this judged nothing"
    );
}
