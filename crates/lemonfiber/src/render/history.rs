//! What lemonfiber has already changed, on a terminal.
//!
//! Newest first, because the change somebody is asking about is nearly always the one
//! they just made. Each line says when, what it did, and — where it is not simply
//! reversible — how far it could be put back and why.
//!
//! Whether a change can be put back is [`lemonfiber_core::rollback`]'s judgement, not
//! this file's. What is here is the wording of it.

use lemonfiber_core::model::{ChangeReport, HistoryReport};

use super::Lines;

/// Everything lemonfiber changed, newest first.
pub(super) fn history(report: &HistoryReport) -> Lines {
    let mut lines = Lines::default();
    if report.changes.is_empty() {
        lines.put("nothing has been changed yet");
        lines.put(String::new());
        lines.put(format!("on record: {}", report.horizon));
        return lines;
    }

    lines.put(format!("{} changes, newest first", report.changes.len()));
    lines.put(String::new());
    for change in &report.changes {
        lines.extend(one(change));
    }
    lines.put(format!("on record: {}", report.horizon));
    lines
}

/// One change, and what putting it back would come to.
fn one(change: &ChangeReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{} — {} ({})",
        change.at, change.did, change.operation
    ));
    lines.put(format!("  on {}", change.target));
    lines.put(format!("  putting it back: {}", putting(&change.reversal)));
    if change.alongside > 1 {
        // The operation is the unit that goes back, so what else would go with it is
        // said on the line rather than counted off the list.
        lines.put(format!(
            "  part of {} changes made together",
            change.alongside
        ));
    }
    if let Some(because) = &change.because {
        lines.put(format!("  because {because}"));
    }
    if let Some(instead) = &change.instead {
        lines.put(format!("  instead {instead}"));
    }
    lines.put(String::new());
    lines
}

/// How far a change could be put back, in the operator's terms rather than the name.
fn putting(reversal: &str) -> &'static str {
    match reversal {
        "whole" => "in full",
        "partial" => "in part",
        _ => "not by lemonfiber",
    }
}
