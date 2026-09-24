//! What this machine keeps of lemonfiber's, on a terminal.
//!
//! The two directories lead, because the answer to "where is all this" is two paths
//! and an operator who reads no further has the whole of it. Each thing under them
//! is named with where it is and why it is kept, and the ones holding a credential
//! say so — that is the sentence that decides how carefully somebody treats a copy.
//!
//! What is *not* lemonfiber's comes last and is the part a removal turns on: an
//! operator about to remove everything needs to have read that the library is not
//! in the list before they agree to it, not afterwards.

use lemonfiber_core::stored::{Removal, Stored};

use super::Lines;

/// What is kept on this machine, and what became of it.
pub(crate) fn kept(report: &Stored) -> Lines {
    let mut lines = Lines::default();
    match &report.removal {
        Removal::Done { gone, left } => return removed(gone, left),
        Removal::NotAsked => lines.put("What lemonfiber keeps on this machine:"),
        Removal::Unconfirmed => {
            lines.put("Removing everything lemonfiber keeps would take all of this:");
        }
    }
    for root in &report.roots {
        lines.spaced(format!("  {}", root.at));
        lines.put(format!("    {}", root.what));
    }
    for entry in &report.kept {
        lines.spaced(format!(
            "  {}{}",
            entry.what,
            if entry.secret {
                " — holds a credential"
            } else {
                ""
            }
        ));
        lines.put(format!("    at   {}", entry.at));
        lines.put(format!("    why  {}", entry.why));
    }
    lines.extend(untouched(report));
    if matches!(report.removal, Removal::Unconfirmed) {
        lines.spaced("Nothing was removed. Add --confirm to remove it.");
    }
    lines
}

/// What is on this machine that lemonfiber does not keep and will not remove.
fn untouched(report: &Stored) -> Lines {
    let mut lines = Lines::default();
    if report.beside.is_empty() {
        return lines;
    }
    lines.spaced("Not lemonfiber's, and never removed:");
    for beside in &report.beside {
        lines.put(format!("  {} — {}", beside.what, beside.why));
    }
    lines
}

/// What a confirmed removal took, and what it could not.
fn removed(gone: &[String], left: &[lemonfiber_core::stored::Left]) -> Lines {
    let mut lines = Lines::default();
    if gone.is_empty() {
        lines.put("Nothing was removed.");
    } else {
        lines.put("Removed:");
        for at in gone {
            lines.put(format!("  {at}"));
        }
    }
    if left.is_empty() {
        lines.spaced("Your library, your downloads and the containers are untouched.");
        return lines;
    }
    lines.spaced("Still here, and each will have to be removed by hand:");
    for still in left {
        lines.put(format!("  {} — {}", still.at, still.why));
    }
    lines
}

#[cfg(test)]
mod tests;
