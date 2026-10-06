//! What a plugin's recipes would do, and how a reading is answered.
//!
//! Apart from the accounts beside it because both are said the same way whichever
//! verb is being read: a recipe is the same calls in the same order on an install and
//! an update, and the name a reading goes by is answered the same way on all three.
//!
//! Every word of a recipe is the plugin author's, and each is drawn through
//! [`plain`], so nothing a manifest says can redraw the line an operator approves on.
//! The manifest's own rules refuse such a word before an install reads it; this is the
//! second wall, not the first.

use lemonfiber_core::plugin::{Adapter, Recipe};
use lemonfiber_core::text::plain;

use super::super::super::Lines;

/// Every recipe, each call in order with the adapter it reaches through, and every
/// value it would send elsewhere with what approving it is written as.
///
/// Nothing at all for a plugin that declares none, which is most of them: a heading
/// over an empty list would be a section somebody has to read to find out it is empty.
pub(super) fn recipes(declared: &[Recipe]) -> Lines {
    let mut lines = Lines::default();
    for recipe in declared {
        lines.spaced(format!(
            "    Recipe {}: {}",
            plain(&recipe.id),
            plain(&recipe.title)
        ));
        lines.put(format!("      {}", plain(&recipe.why)));
        for (at, step) in recipe.steps.iter().enumerate() {
            lines.put(format!(
                "      {}. {} {} {}{}",
                at + 1,
                plain(&step.method),
                plain(&step.to),
                plain(&step.path),
                through(step.adapter.as_ref())
            ));
        }
        for pair in &recipe.pairs {
            let whose = if pair.origin.is_empty() {
                String::new()
            } else {
                format!(" ({})", plain(&pair.origin))
            };
            lines.put(format!(
                "      sends {}{whose} to {} — approve with --approve {}",
                plain(&pair.value),
                plain(&pair.to),
                plain(&pair.approval)
            ));
        }
    }
    lines
}

/// The adapter a call reaches through, said to be lemonfiber's, or nothing for a call
/// to a name outside the stack.
fn through(adapter: Option<&Adapter>) -> String {
    adapter.map_or_else(String::new, |one| {
        format!(", through lemonfiber's {} adapter", word(*one))
    })
}

/// The adapter's own word, as the published set spells it.
fn word(adapter: Adapter) -> String {
    serde_json::to_value(adapter.kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// How a run that changed nothing closes: that it changed nothing, and how the reading
/// it was is answered — the same command again, with the name the reading goes by and
/// an approval for every value it would send elsewhere.
///
/// The same words for all three verbs, so a script or a person that has answered one
/// has answered them all. A report carrying no offer says only that nothing changed,
/// because there is nothing to answer.
pub(super) fn unanswered(
    verb: &str,
    agreement: Option<&str>,
    approvals: &[&str],
    rehearsed: bool,
) -> Lines {
    let mut lines = Lines::default();
    let nothing = if rehearsed {
        "Nothing was written, because this was a rehearsal."
    } else {
        "Nothing has been changed."
    };
    let Some(agreement) = agreement else {
        lines.spaced(nothing);
        return lines;
    };
    lines.spaced(format!(
        "{nothing} To {verb} it, run the same command again, answering this offer by name:"
    ));
    let mut answer = format!("  --offer {}", plain(agreement));
    for pair in approvals {
        answer.push_str(" --approve ");
        answer.push_str(&plain(pair));
    }
    lines.put(answer);
    lines
}

#[cfg(test)]
mod tests;
