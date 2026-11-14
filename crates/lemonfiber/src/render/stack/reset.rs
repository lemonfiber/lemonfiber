//! What a full reset reverts, or would.

use lemonfiber_core::model::ResetReport;

use super::super::Lines;

/// What a full reset did, or — until confirmed — would do: the edits it reverts to
/// lemonfiber's own state, named with what is lost, so nothing is discarded unseen.
pub(in crate::render) fn reset(report: &ResetReport) -> Lines {
    let mut lines = reverting(report);
    lines.extend(super::super::moving::guards(&report.guards));
    lines
}

/// The edits and connections a reset reverts, or would.
fn reverting(report: &ResetReport) -> Lines {
    let mut lines = Lines::default();
    if report.reverted.is_empty() && report.reverted_connections.is_empty() {
        lines.put("Nothing to reset — the stack is already lemonfiber's own.");
        return lines;
    }
    let count = report.reverted.len() + report.reverted_connections.len();
    if report.confirmed {
        lines.put(format!("Reverted {count} change(s) to lemonfiber's state:"));
    } else {
        lines.put(format!(
            "A reset would revert these {count} change(s) to lemonfiber's state — run \
             `lemonfiber reset --offer {}` to do it:",
            report.offer
        ));
    }
    // The service connections whose category drifted, named as they read in a seed report.
    for connection in &report.reverted_connections {
        lines.put(format!("  · {connection}"));
    }
    // The hand-edited stack files, each with the diff of what is lost.
    for edit in &report.reverted {
        lines.spaced(format!("  {}", edit.path));
        lines.block(&edit.diff);
    }
    lines
}
