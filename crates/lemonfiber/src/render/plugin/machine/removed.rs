//! What taking a plugin off the machine came to, or would come to.

use lemonfiber_core::plugin::{Removal, Unfilled};

use super::super::super::Lines;

/// What taking a plugin off the machine came to, or would come to.
///
/// **What it would leave without comes before what it put back**, because that is the
/// half an operator can still act on. What went back is an account; what nothing is
/// left filling is a decision they may want to take differently.
pub(super) fn removal(one: &Removal) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{} {}:",
        if one.removed {
            "Removed"
        } else {
            "Would remove"
        },
        one.plugin
    ));
    // Past tense only where it is true. A container the engine would not take off is
    // named below as still standing, and a line above it saying it stopped would be
    // the page contradicting itself about the one thing an operator came to check.
    let stayed = one
        .went_back
        .left
        .iter()
        .any(|left| left.target == one.plugin);
    lines.spaced(format!(
        "    {} {}",
        match (one.removed, stayed) {
            (false, _) => "Would stop:",
            (true, false) => "Stopped:",
            (true, true) => "Asked to stop:",
        },
        one.interrupts.join(", ")
    ));
    lines.extend(leaves(&one.leaves, one.removed));
    lines.extend(super::reversal(&one.went_back));
    lines
}

/// Every capability the machine would have nothing filling afterwards.
///
/// **The empty case is worded and it is the common one.** A plugin that fills nothing
/// the stack asks for takes nothing away with it, which is the thing an operator most
/// wants to know before agreeing — and a blank section says it least clearly.
fn leaves(going: &[Unfilled], removed: bool) -> Lines {
    let mut lines = Lines::default();
    if going.is_empty() {
        lines.spaced(
            "    Nothing on this machine is left asking for something with nothing to fill it.",
        );
        return lines;
    }
    lines.spaced(format!(
        "    What {} nothing to fill it:",
        if removed { "now has" } else { "would have" }
    ));
    for one in going {
        lines.put(format!(
            "      {} — {} is the only thing filling it",
            one.capability, one.filled_by
        ));
    }
    lines
}
