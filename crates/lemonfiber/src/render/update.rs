//! What an update would change, and what a run of one came to.
//!
//! Two readings of one report, and which one is worth printing is decided by
//! whether anything was agreed to: before, the whole point is the list of steps and
//! what each of them costs; after, it is which services moved and how to get back
//! from the ones that did not.
//!
//! A stack already on every pinned version ends after one line. Nothing is offered,
//! nothing is suggested, and there is no second sentence asking again — staying on
//! the versions you have is a position this product supports rather than one it
//! argues with.

use lemonfiber_core::migration::version::Jump;
use lemonfiber_core::update::run::Report;
use lemonfiber_core::update::{Applied, Change, Ending, Reversal, State};

use super::Lines;

/// What an update would change, or what one did.
pub(crate) fn update(report: &Report) -> Lines {
    let mut lines = Lines::default();
    if report.state == State::Current {
        lines.put("Every service is on the version this build of lemonfiber pins.");
        return lines;
    }

    lines.put(heading(report.state));
    for change in &report.changes {
        lines.extend(step(change));
    }
    lines.extend(transfers(&report.in_flight));
    if report.confirmed {
        lines.extend(carried(report));
    } else {
        // Before the agreement and not after it. What the pins came from is what
        // somebody deciding is weighing; somebody reading a finished run wants to
        // know which services moved, and this would be between them and it.
        lines.extend(super::changelog::brought(&report.changelog));
        lines.spaced("Take them with:  lemonfiber update --confirm");
    }
    lines
}

/// The line the report opens with, which is the one word it came to.
fn heading(state: State) -> &'static str {
    match state {
        State::Current | State::UpdatesAvailable => {
            "Newer versions are pinned than what is running:"
        }
        State::Updated => "Updated:",
        State::Partial => {
            "Partly updated — the run stopped at the first service that did not come back:"
        }
        State::Failed => "Nothing was updated — the run stopped at the first service:",
    }
}

/// One step, with the size of it and what taking it means.
fn step(change: &Change) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "  {}  {} to {}  ({})",
        change.service,
        change.current,
        change.target,
        size(change.jump)
    ));
    if change.jump == Jump::Major {
        lines.put(
            "    A first-number change. Versions that move it carry changes that break \
             configurations far more often than the two behind it.",
        );
    }
    if change.refused {
        lines.put(format!("    Refused: {}", change.because));
        return lines;
    }
    if change.irreversible {
        lines.put(format!("    {}", change.because));
    }
    lines
}

/// How large a step reads, in the words an operator weighs it in.
fn size(jump: Jump) -> &'static str {
    match jump {
        Jump::Major => "major",
        Jump::Minor => "minor",
        Jump::Patch => "patch",
        Jump::Untellable => "size unknown",
    }
}

/// What is still coming down, where anything is.
fn transfers(active: &[String]) -> Lines {
    let mut lines = Lines::default();
    if active.is_empty() {
        return lines;
    }
    lines.spaced("Still coming down, and updating stops the download clients:");
    for one in active {
        lines.put(format!("  {one}"));
    }
    lines.put("Let them finish first with:  lemonfiber update --confirm --wait");
    lines
}

/// What the run itself did: the backup it took, each service, and where it stopped.
fn carried(report: &Report) -> Lines {
    let mut lines = Lines::default();
    if let Some(path) = &report.backup {
        lines.spaced(format!("Backed up to {path} before anything was started."));
    }
    if !report.applied.is_empty() {
        lines.spaced("Service by service:");
    }
    for one in &report.applied {
        lines.extend(service(one));
    }
    if let Some(halted) = &report.halted {
        lines.spaced(halted.clone());
    }
    for edit in &report.stack_edits {
        lines.spaced(format!(
            "{} is yours — it was left exactly as you set it, and this is what was held back:",
            edit.path
        ));
        lines.block(&edit.diff);
    }
    lines
}

/// What became of one service, and how to put it back.
fn service(one: &Applied) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "  {}  {} to {}  — {}",
        one.service,
        one.from,
        one.to,
        ended(one.ending)
    ));
    if let Some(detail) = &one.detail {
        lines.put(format!("    {detail}"));
    }
    if one.ending != Ending::Updated {
        lines.put(format!("    {}", back(one.reversal)));
    }
    lines
}

/// How one service ended, in one phrase.
fn ended(ending: Ending) -> &'static str {
    match ending {
        Ending::Updated => "updated",
        Ending::NotFetched => "not fetched, so it is still on the version it was",
        Ending::NotStarted => "started and did not come back",
        Ending::NotReached => "not reached, so it is still on the version it was",
    }
}

/// The one way back that can actually work for a service that ended this way.
fn back(reversal: Reversal) -> &'static str {
    match reversal {
        Reversal::Rollback => {
            "Nothing of it opened the newer image, so starting it again puts it back as it was."
        }
        Reversal::Restore => {
            "It opened its state on the newer image, so the backup is the way back: \
             lemonfiber restore <archive>"
        }
    }
}

#[cfg(test)]
mod tests;
