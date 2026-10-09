//! What proving an installed plugin again came to, on a terminal.

use lemonfiber_core::plugin::{Evidence, Reproof};

use super::super::super::Lines;

/// What proving a plugin again came to, or would.
pub(super) fn proved(proof: &Reproof) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{} {}:",
        match (proof.asked, proof.cleared) {
            (false, _) => "Would prove",
            (true, true) => "Proved",
            (true, false) => "Did not prove",
        },
        proof.plugin
    ));
    for kept in &proof.kept {
        lines.put(format!(
            "    kept       {} {} at {}: {}",
            kept.capability, kept.operation, kept.at, kept.why
        ));
    }
    lines.extend(super::proving(
        &proof.proofs,
        proof.asked.then_some(Evidence::Service),
        proof.asked,
    ));
    lines.spaced(match (proof.asked, proof.cleared) {
        (false, _) => "    A pass clears every answer kept against it, and it fills what it provides again.",
        (true, true) => "    Every answer kept against it is cleared, and it fills what it provides again.",
        (true, false) => "    What is kept against it stays, and it fills none of that until a proof it passes clears it.",
    });
    lines
}
