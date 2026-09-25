//! How a walkthrough reads once it is over.
//!
//! Three endings, and they owe the operator different things. One that worked owes them
//! somewhere to go next, because the moment the first thing plays is the moment they have
//! nothing to do. One that stopped owes them the step, the services' own words, and one
//! action — a fault report they have to research is a fault report they abandon. And one
//! still running owes them the truth that walking away costs nothing.

use lemonfiber_core::model::WalkthroughReport;
use lemonfiber_core::walkthrough::{Handover, Stopped};
use lemonfiber_core::PRODUCT;

use super::super::Lines;

/// The whole ending, for a person.
pub(crate) fn ending(report: &WalkthroughReport) -> Lines {
    let mut lines = Lines::default();
    if report.already_here {
        return already_here(report);
    }
    if let Some(stopped) = &report.stopped {
        return stop(stopped);
    }
    if report.in_background {
        return still_going(report);
    }

    let named = report.item.clone().unwrap_or_default();
    lines.spaced(format!("That is {named}, all the way through."));
    if let Some(link) = report.link {
        lines.put(format!("  {}", link.consequence()));
        if let Some(remedy) = link.remedy() {
            lines.put(format!("  {remedy}"));
        }
    }
    if let Some(handover) = &report.handover {
        lines.extend(next(handover));
    }
    lines
}

/// Where a finished walkthrough leaves the operator.
fn next(handover: &Handover) -> Lines {
    let mut lines = Lines::default();
    lines.spaced("What next:");
    for step in &handover.next {
        lines.put(format!("  · {}", step.said()));
        lines.put(format!("      {}", step.how()));
    }
    lines
}

/// A walkthrough that stopped: the step, what the services said, and the one thing to try.
fn stop(stopped: &Stopped) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(format!("Stopped at: {}", stopped.step.said()));
    lines.put(format!("  {}", stopped.reason.said()));
    if !stopped.logs.is_empty() {
        lines.put("");
        lines.put(format!("  What {} was saying:", stopped.step.done_by()));
        for said in &stopped.logs {
            lines.put(format!("    {said}"));
        }
    }
    lines.spaced(format!("  → {}", stopped.remedy));
    // Only where something is genuinely wrong: sending an operator to fix a stack that is
    // working, because an indexer had nothing, is the wrong lesson twice over.
    if !stopped.reason.is_a_fault() {
        lines.put("  Nothing here is broken — this is what the indexers had.");
    }
    lines
}

/// The stack already had it — detected rather than acquired again.
fn already_here(report: &WalkthroughReport) -> Lines {
    let mut lines = Lines::default();
    let named = report.item.clone().unwrap_or_default();
    lines.spaced(format!(
        "{named} is already here — nothing was fetched again."
    ));
    if !report.suggestions.is_empty() {
        lines.put("");
        lines.put("Try one of these instead:");
        for suggestion in &report.suggestions {
            lines.put(format!("  · {suggestion}"));
        }
    }
    lines
}

/// Still coming, and the operator has their terminal back.
fn still_going(report: &WalkthroughReport) -> Lines {
    let mut lines = Lines::default();
    let named = report.item.clone().unwrap_or_default();
    lines.spaced(format!(
        "{named} is still downloading — it will finish on its own."
    ));
    lines.put("Nothing was cancelled by stopping here.");
    // Quoted the way the other trace link is quoted, and by the same function: this is a
    // command the operator copies, and the title in it is the catalogue's rather than
    // ours. Two spellings of one command line would drift, and the one that drifted
    // would be the one nobody pastes until the day a title has an apostrophe in it.
    lines.spaced(format!(
        "  Follow it:  {PRODUCT} trace {}",
        crate::render::trace::one_argument(&named)
    ));
    lines
}

#[cfg(test)]
mod tests;
