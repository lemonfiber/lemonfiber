//! What a repairing run offered, and what became of it.
//!
//! The outcome leads every line, because that is the question being answered: not whether
//! lemonfiber ran something, but whether the fault is gone.

use lemonfiber_core::app::repair::{Report, Reversal};
use lemonfiber_core::journal::Action;
use lemonfiber_core::repair::{Outcome, ASK_FOR_REPAIRS};

use super::Lines;

/// What was offered and what was done about it.
///
/// For a person only. What a parser reads is the envelope every other answer is
/// rendered into, which is the one both surfaces put on the wire.
pub(crate) fn mended(report: &Report) -> Lines {
    let mut lines = Lines::default();
    // Said first, because it is the one thing here the operator has to act on themselves.
    for beyond in &report.beyond {
        lines.put(format!(
            "{} has outlasted every repair for it.",
            beyond.check
        ));
        lines.remedy(&beyond.remedy, "  ");
    }
    if report.offered.is_empty() && report.beyond.is_empty() {
        lines.put("There is nothing here lemonfiber can put right itself.");
        return lines;
    }
    if report.offered.is_empty() {
        return lines;
    }
    if !report.acted {
        lines.put("These could be put right:");
        for repair in &report.offered {
            lines.put(format!("  {}", repair.does));
        }
        lines.spaced(format!(
            "Nothing has been changed. Run `{ASK_FOR_REPAIRS}` to be asked about each."
        ));
        return lines;
    }
    for done in &report.mended {
        lines.put(format!("{} — {}", said(&done.outcome), done.repair.does));
    }
    lines
}

/// What was put back, or that there was nothing to put back.
///
/// One rendering for both tenses rather than two, because the two lists mean the same
/// thing either way: what goes back here, and what does not go back here without
/// something else being true. Only the heading moves, and a second renderer for a
/// rehearsal would be a second account of one report.
pub(crate) fn reversed(report: &Reversal) -> Lines {
    let (undos, left) = (&report.reversed, &report.left);
    let mut lines = Lines::default();
    if undos.is_empty() && left.is_empty() {
        lines.put("There is no repair to put back.");
        return lines;
    }
    if !undos.is_empty() {
        lines.put(if report.rehearsed {
            "Would put back:"
        } else {
            "Put back:"
        });
        for undo in undos {
            lines.put(format!("  {} — {}", undo.target, restoring(&undo.action)));
        }
    }
    // Said even when everything went back would be noise; said when anything did not is
    // the whole point of the report. An operator who asked for five things and got three
    // finds out here rather than by going and looking.
    if !left.is_empty() {
        lines.put(if report.rehearsed {
            "Would depend on something else:"
        } else {
            "Still as it was:"
        });
        for standing in left {
            lines.put(format!("  {} — {}", standing.target, standing.because));
        }
    }
    // What going back means beyond going back. Neither list above can carry it: it did
    // not fail, so it is not what was left, and saying only that it went back would send
    // somebody looking for their files at an address that no longer names them.
    if !report.noted.is_empty() {
        lines.put("Worth knowing:");
        for note in &report.noted {
            lines.put(format!("  {}", note.because));
        }
    }
    if report.rehearsed {
        lines.spaced("Nothing has been put back. Run it without --dry-run to do it.");
    }
    lines
}

/// What one reversal did, in the words of the thing it acted on.
fn restoring(action: &Action) -> String {
    match action {
        Action::Restore {
            key,
            value: Some(value),
            ..
        } => format!("{key} back to {value}"),
        // Nothing was there before, so putting it back means taking it away again.
        Action::Restore {
            key, value: None, ..
        } => format!("{key} removed, as it was"),
        Action::Remove { resource, id } => format!("{resource} {id} removed"),
        Action::Reconfigure {
            resource,
            field,
            value: Some(value),
            ..
        } => format!("{resource}'s {field} back to {value}"),
        // Nothing was there before, so putting it back means clearing it again.
        Action::Reconfigure {
            resource, field, ..
        } => format!("{resource}'s {field} cleared, as it was"),
        Action::Delete { path } => format!("{path} removed"),
        Action::Withdraw { owner, path, .. } => format!("{owner}'s region taken out of {path}"),
        Action::Repin { previous, .. } => format!("the version pinned back to {previous}"),
    }
}

/// How one outcome reads.
///
/// A repair that ran and left the fault standing says so plainly rather than borrowing the
/// word for one that worked: the difference is the entire point of asking the check again,
/// and a report that blurred it would undo the care taken to establish it.
fn said(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Fixed => "fixed".to_owned(),
        Outcome::FixFailed => "still wrong afterwards".to_owned(),
        // The state it was left in is the most useful sentence in a failed repair, so it
        // travels with the verdict rather than being flattened out of it.
        Outcome::Stopped { leaving } => format!("stopped partway — {leaving}"),
        Outcome::Declined => "left alone".to_owned(),
        Outcome::WouldOverwrite => "refused, it would overwrite your own change".to_owned(),
        // A different sentence from the one above, and the difference is the point: that
        // one is lemonfiber declining to write over a change it can see, and this is
        // lemonfiber obeying an instruction it was given.
        Outcome::Unmanaged => "left alone, you declared this unmanaged".to_owned(),
    }
}

#[cfg(test)]
mod tests;
