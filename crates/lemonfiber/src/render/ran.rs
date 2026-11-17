//! What one minute's look at a run-and-exit service came to, as `lemonfiber run` says it.

use lemonfiber_core::model::RunReport;

use super::news::moment;
use super::Lines;

/// Whether it ran and how that run ended, why nothing was looked at, or that nothing was
/// due.
pub(super) fn ran(report: &RunReport) -> Lines {
    let mut lines = Lines::default();
    let service = &report.service;
    if let Some(held) = &report.held {
        lines.put(format!("{service} was not looked at: {held}."));
        return lines;
    }
    let Some(ran) = &report.ran else {
        lines.put(if report.due {
            format!("{service} is due, and would run now.")
        } else {
            format!("None of {service}'s times fell due.")
        });
        return lines;
    };
    lines.put(match ran.exit {
        Some(0) => format!(
            "{service} ran, from {} to {}.",
            moment(ran.started.seconds()),
            moment(ran.ended.seconds())
        ),
        Some(exit) => format!("{service} ran and exited with {exit}."),
        None => format!("{service} ended before it exited."),
    });
    if !ran.succeeded() {
        for line in &ran.log {
            lines.put(format!("  {line}"));
        }
    }
    if report.skipped > 0 {
        lines.put(format!(
            "{} time(s) fell due while it ran, and were skipped.",
            report.skipped
        ));
    }
    lines
}

#[cfg(test)]
mod tests;
