//! Every credential this stack holds, on a terminal, with none of their values.
//!
//! The inventory leads because it is the answer to the question that was asked, and
//! each line says the four things an operator needs: what it is, what authenticates
//! with it, where the value lives, and where it stands. The advisories come after,
//! not beside — one credential that has gone stale should not push the other twelve
//! off the top of a screen.
//!
//! What the storage protects against is printed every time, both halves of it. An
//! operator reading this list is exactly the operator about to decide whether a copy
//! of that file is safe to put somewhere, and the answer is longer than "yes".

use lemonfiber_core::credential::{Inventory, Reach, Revealed, Rotation, Settled};

use super::Lines;

/// Everything held, and what became of anything asked about one of them.
pub(crate) fn listing(inventory: &Inventory) -> Lines {
    let mut lines = Lines::default();
    lines.put("The credentials this stack holds. None of their values is shown.");
    for held in &inventory.held {
        lines.spaced(format!("  {} — {}", held.name, held.state.as_str()));
        if let Some(plugin) = held.plugins() {
            lines.put(format!("    brought by   the plugin {plugin}"));
        }
        lines.put(format!("    recorded as  {}", held.setting));
        lines.put(format!("    kept in      {}", held.location));
        for consumer in &held.consumers {
            lines.put(format!("    used by      {consumer}"));
        }
    }
    lines.extend(advisories(inventory));
    lines.extend(protection(inventory));
    if let Some(rotated) = &inventory.rotated {
        lines.extend(rotation(rotated));
    }
    if let Some(revealed) = &inventory.revealed {
        lines.extend(reveal(revealed));
    }
    lines
}

/// What is worth saying about the ones that are not simply working.
fn advisories(inventory: &Inventory) -> Lines {
    let mut lines = Lines::default();
    let said = inventory.advisories();
    if said.is_empty() {
        return lines;
    }
    lines.spaced("Worth knowing. None of this expires on its own:");
    for one in said {
        lines.put(format!("  {one}"));
    }
    lines
}

/// What keeping them in files does, and does not, protect against.
fn protection(inventory: &Inventory) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(inventory.protection.summary.clone());
    lines.spaced("That protects against:");
    for one in &inventory.protection.against {
        lines.put(format!("  {one}"));
    }
    lines.spaced("It does not protect against:");
    for one in &inventory.protection.not_against {
        lines.put(format!("  {one}"));
    }
    lines
}

/// What became of a rotation, and of every consumer it had to reach.
fn rotation(rotated: &Rotation) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(match &rotated.settled {
        Settled::Replaced { observed } => format!(
            "{} was replaced. {observed}. The old value is gone.",
            rotated.credential
        ),
        Settled::Refused { detail } => format!("{} was not replaced: {detail}", rotated.credential),
        Settled::Unproven { detail } => {
            format!("{} was not replaced: {detail}", rotated.credential)
        }
        Settled::Elsewhere { detail } => format!("{}: {detail}", rotated.credential),
        Settled::Unknown { known } => format!(
            "Nothing here is called `{}`. What is: {}.",
            rotated.credential,
            known.join(", ")
        ),
        Settled::Rehearsed {
            detail, location, ..
        } => format!(
            "{} would be replaced where it is kept, in {location}. {detail}",
            rotated.credential
        ),
    });
    if let Settled::Rehearsed { afterwards, .. } = &rotated.settled {
        owed(&mut lines, afterwards);
    }
    if rotated.consumers.is_empty() {
        return lines;
    }
    lines.spaced("Everything that authenticates with it:");
    for one in &rotated.consumers {
        lines.put(format!("  {} — {}", one.consumer, reached(&one.reach)));
    }
    let stranded = rotated.stranded();
    if !stranded.is_empty() {
        lines.spaced(format!(
            "Still holding the old value, and it no longer works: {}.",
            stranded.join(", ")
        ));
    }
    lines
}

/// What would still have to happen before every consumer held a replacement.
///
/// Its own heading rather than folded into the sentence above it, because it is the
/// half an operator plans around: a rotation that lands and leaves three consumers on
/// the old value is one they find out about when something stops working. Nothing at
/// all where nothing is owed, since an empty heading reads as a list somebody forgot
/// to fill in.
fn owed(lines: &mut Lines, afterwards: &[String]) {
    if afterwards.is_empty() {
        return;
    }
    lines.spaced("Afterwards, before everything holds the replacement:");
    for step in afterwards {
        lines.put(format!("  {step}"));
    }
}

/// How far the replacement reached one consumer, in one clause.
fn reached(reach: &Reach) -> String {
    match reach {
        Reach::Updated => "has it".to_owned(),
        Reach::Pending { detail } => format!("still to be given it: run `{detail}`"),
        Reach::Failed { detail } => format!("could not be given it: {detail}"),
    }
}

/// One credential, printed or explained.
///
/// The warning goes first either way. An operator who asked for this and changed
/// their mind reads it before the value scrolls past, rather than after.
fn reveal(revealed: &Revealed) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(revealed.warning.clone());
    if let Some(value) = &revealed.value {
        lines.spaced(format!("  {}: {value}", revealed.name));
    }
    lines
}

#[cfg(test)]
mod tests;
