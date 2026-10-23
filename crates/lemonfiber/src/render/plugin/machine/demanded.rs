//! What asking for one of a plugin's demand recipes came to, on a terminal.

use lemonfiber_core::error::Came;
use lemonfiber_core::plugin::Demand;
use lemonfiber_core::text::plain;

use super::super::super::Lines;

/// The recipe asked for, as the reading says it, and once it ran what each step came to.
pub(super) fn demanded(demand: &Demand) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{} {}'s recipe {}:",
        if demand.ran.is_some() {
            "Ran"
        } else {
            "Would run"
        },
        plain(&demand.plugin),
        plain(&demand.recipe.id)
    ));
    lines.extend(super::answering::recipes(std::slice::from_ref(
        &demand.recipe,
    )));
    if let Some(ran) = &demand.ran {
        lines.spaced("    What each step came to:");
        for step in &ran.steps {
            lines.put(format!(
                "      {} to {}: {}",
                plain(&step.step),
                plain(&step.to),
                came(step.came)
            ));
        }
    }
    lines
}

/// What one step came to, in words.
const fn came(came: Came) -> &'static str {
    match came {
        Came::Answered => "answered as it should",
        Came::Skipped => "skipped, because its guard did not hold",
        Came::NotReached => "not made, because an earlier step did not hold",
        Came::Unreachable => "nothing answered it",
        Came::Refused => "not sent, because its host is not out on the internet",
        Came::Withheld => "not sent, because a value it carries may not go there",
        Came::Unexpected => "answered with something other than what it expects",
        Came::Uncaptured => "its answer held nothing where a capture reads",
        Came::Oversized => "its answer was larger than a recipe reads",
    }
}

#[cfg(test)]
mod tests;
