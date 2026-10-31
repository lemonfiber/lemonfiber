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

use lemonfiber_core::plugin::{Installed, Recipe, Sharing, Taking};
use lemonfiber_core::text::plain;

use super::super::super::Lines;

/// Every recipe, each call in order with the API it reaches its service in, and every
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
        if recipe.on == lemonfiber_core::plugin::On::Demand {
            lines.put("      runs only when asked for by name");
        }
        for input in &recipe.inputs {
            lines.put(format!(
                "      asks {}: {}{}",
                plain(&input.name),
                plain(&input.ask),
                if input.secret {
                    " (a secret, taken without showing it)"
                } else {
                    ""
                }
            ));
        }
        for (at, step) in recipe.steps.iter().enumerate() {
            lines.put(format!(
                "      {}. {} {} {}{}",
                at + 1,
                plain(&step.method),
                plain(&step.to),
                plain(&step.path),
                speaking(step.native.as_deref())
            ));
        }
        for pair in &recipe.pairs {
            let whose = if pair.origin.is_empty() {
                String::new()
            } else {
                format!(" ({})", plain(&pair.origin))
            };
            let asked = pair.approval.as_deref().map_or_else(
                || " — inside the stack, nothing to approve".to_owned(),
                |approval| format!(" — approve with --approve {}", plain(approval)),
            );
            lines.put(format!(
                "      sends {}{whose} to {}{asked}",
                plain(&pair.value),
                plain(&pair.to),
            ));
            if let Some(release) = &pair.release {
                lines.put(format!(
                    "        released from {}: {}",
                    plain(pair.from.as_deref().unwrap_or_default()),
                    plain(release)
                ));
            }
        }
    }
    lines
}

/// Every service taking a privileged shape, with what it is given and, before it is
/// given it, what approving it is written as.
pub(super) fn taking(taken: &[Taking], acted: bool) -> Lines {
    let mut lines = Lines::default();
    if taken.is_empty() {
        return lines;
    }
    lines.spaced(format!(
        "    The privileged shape it {}, approved apart from the offer:",
        if acted { "was given" } else { "would be given" }
    ));
    for one in taken {
        let asked = if acted {
            String::new()
        } else {
            format!(" — approve with --approve {}", plain(&one.approval))
        };
        lines.put(format!(
            "      {} takes {}: the {} capability and the {} device{asked}",
            plain(&one.service),
            one.shape.name(),
            one.grants.join(", "),
            one.devices.join(", "),
        ));
    }
    lines
}

/// Each adapter that writes the files of the service it fronts, and where it reaches them.
pub(super) fn sharing(shared: &[Sharing], acted: bool) -> Lines {
    let mut lines = Lines::default();
    if shared.is_empty() {
        return lines;
    }
    lines.spaced(format!(
        "    The files of another service it {}:",
        if acted { "writes" } else { "would write" }
    ));
    for one in shared {
        lines.put(format!(
            "      {} writes {}'s own files, its whole configuration directory mounted at {}",
            plain(&one.adapter),
            plain(&one.upstream),
            plain(&one.at),
        ));
    }
    lines
}

/// The API a call reaches its service in, in the declaring plugin's word, or nothing
/// where none is declared.
fn speaking(native: Option<&str>) -> String {
    native.map_or_else(String::new, |api| format!(", in its {} API", plain(api)))
}

/// Every credential of another service the plugin is granted, with the capability it is
/// for, whose it is and, before it is granted, what approving it is written as.
pub(super) fn granting(would: &Installed, acted: bool) -> Lines {
    let mut lines = Lines::default();
    let (plugin, granted) = (would.plugin.as_str(), &would.granted);
    if granted.is_empty() {
        return lines;
    }
    lines.spaced(format!(
        "    The credentials it {}, each approved apart from the offer:",
        if acted {
            "was granted"
        } else {
            "would be granted"
        }
    ));
    for grant in granted {
        let asked = if acted {
            String::new()
        } else {
            format!(
                " — approve with --approve {}",
                plain(&grant.approval(plugin))
            )
        };
        lines.put(format!(
            "      {}, {}'s, for {}{asked}",
            plain(&grant.credential),
            plain(&grant.of),
            plain(&grant.serves),
        ));
    }
    lines
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
