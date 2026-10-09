//! What a plugin's recordings say about the contracts its adapters speak, on a terminal.

use lemonfiber_core::plugin::Conformed;

use super::super::Lines;
use super::{came_to, counted, document};

/// What judging a plugin's conformance recordings came to, in whichever form was asked.
pub(crate) fn conformed(read: &Conformed, json: bool) -> Option<Lines> {
    if json {
        return serde_json::to_string_pretty(read)
            .ok()
            .as_deref()
            .map(document);
    }
    Some(judged(read))
}

/// Each case under the contract it is of, and what the whole comes to.
fn judged(read: &Conformed) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{} — what its recordings say about the contracts it speaks",
        read.id
    ));
    if !read.refusals.is_empty() {
        lines.spaced(format!(
            "Refused, {}:",
            counted(read.refusals.len(), "violation")
        ));
        for refusal in &read.refusals {
            lines.put(format!("  {} — {}", refusal.location, refusal.message));
        }
    }
    let mut contract = None;
    for one in &read.cases {
        if contract != Some(&one.contract) {
            lines.spaced(format!("  {}", one.contract));
            contract = Some(&one.contract);
        }
        lines.put(format!("    {:<26} {}", one.case, came_to(&one.verdict)));
    }
    lines.spaced(if read.conforms && read.cases.is_empty() {
        "It speaks no contract, so there is nothing to judge."
    } else if read.conforms {
        "It conforms to every contract it speaks."
    } else {
        "It does not conform: a plugin whose recordings fail or cannot prove a case fills \
         none of that capability."
    });
    lines
}

#[cfg(test)]
mod tests;
