//! The services held back until the operator gives what they need to start, said the one
//! way status, the doctor and every rehearsal say them.

use lemonfiber_core::model::{Waiting, Wanted};
use lemonfiber_core::text::plain;

use super::Lines;

/// Each service held back, with everything it waits for, and how to give it.
pub(super) fn waiting(held: &[Waiting]) -> Lines {
    let mut lines = Lines::default();
    if held.is_empty() {
        return lines;
    }
    lines.spaced("Held back until you give what they need to start:");
    for one in held {
        lines.put(format!("  {}", plain(&one.said())));
    }
    if held
        .iter()
        .flat_map(|one| &one.wants)
        .any(|want| matches!(want, Wanted::Setting { .. }))
    {
        lines.put("Give a setting with lemonfiber settings set <setting>=<value>.");
    }
    lines
}

#[cfg(test)]
mod tests;
