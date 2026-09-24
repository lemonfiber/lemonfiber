//! What the diagnostic checks found, finding by finding.
//!
//! One of the renderers, its own file so each answer's shape is read on its own.
//! Every one of them builds lines and hands them back; the printer is at the edge.

use lemonfiber_core::doctor::{Overall, Verdict};
use lemonfiber_core::error::{Problem, State};
use lemonfiber_core::model::DoctorReport;
use lemonfiber_core::repair::{mendable, ASK_FOR_REPAIRS};

use super::Lines;

/// What the diagnostic checks found, finding by finding.
///
/// Each finding leads with a mark that reads at a glance and the plain evidence
/// behind it; a non-passing one carries the reason and what to do, because a
/// finding without a remedy is a fault report rather than a diagnosis.
pub(super) fn diagnosis(report: &DoctorReport) -> Lines {
    let mut lines = Lines::default();
    for finding in &report.findings {
        let title = &titled(finding);
        match &finding.verdict {
            Verdict::Pass { note } => match note {
                Some(note) => lines.put(format!("  ✓ {title}   {note}")),
                None => lines.put(format!("  ✓ {title}")),
            },
            // An answered choice keeps its line and loses its lead: still there,
            // still saying what it costs, no longer repeating the remedy for
            // something the operator has already decided against doing.
            Verdict::Warn(problem) if problem.state == State::Suppressed => {
                lines.put(format!("  · {title}   {} (answered)", problem.summary));
            }
            Verdict::Warn(problem) => {
                lines.put(format!("  ! {title}   {}", problem.summary));
                lines.extend(remedies(problem));
            }
            Verdict::Fail(problem) => {
                lines.put(format!("  ✗ {title}   {}", problem.summary));
                lines.extend(remedies(problem));
            }
            Verdict::Unverified { reason, remedy } => {
                lines.put(format!("  ? {title}   UNVERIFIED"));
                lines.put(format!("      {reason}"));
                lines.put(format!("      → {}", remedy.action));
                if let Some(detail) = &remedy.detail {
                    lines.put(format!("        {detail}"));
                }
            }
            Verdict::Skipped { reason } => {
                lines.put(format!("  – {title}   skipped: {reason}"));
            }
        }

        // What the service said for itself, under the finding it explains. A check
        // can say a service is not answering; only the service can say why, and an
        // operator who has to go and fetch that has been handed a fault report
        // rather than a diagnosis.
        lines.extend(said(finding.said.as_deref()));
    }

    // Said once, and only where a row is marked, so what the unmarked rows are is never
    // left to be inferred from the marked ones.
    if report
        .findings
        .iter()
        .any(|finding| finding.origin != lemonfiber_core::origin::Origin::Bundled)
    {
        lines.spaced(
            "A check marked with where it came from is not this build's own; every other is.",
        );
    }
    lines.spaced(overall(report.overall));
    lines
}

/// A finding's title, with where it came from beside it wherever that is not this
/// build.
///
/// Beside the title rather than in a section of its own, so reading the row and reading
/// whose it is are the same act — the operator it matters most to is the one who did
/// not think to ask. This build's own rows are left unmarked because they are nearly
/// every row, and a word repeated on forty lines is a word nobody reads; the line at
/// the foot says so wherever anything is marked.
fn titled(finding: &lemonfiber_core::doctor::Finding) -> String {
    match &finding.origin {
        lemonfiber_core::origin::Origin::Bundled => finding.title.clone(),
        lemonfiber_core::origin::Origin::Plugin { named } => {
            format!("{} (from plugin {named})", finding.title)
        }
        other => format!("{} (from {})", finding.title, other.as_str()),
    }
}

/// A service's own recent output, indented under the finding it belongs to.
///
/// Nothing at all where there is none: a heading with no lines under it promises
/// evidence that is not there, which is worse than saying nothing.
fn said(output: Option<&str>) -> Lines {
    let mut lines = Lines::default();
    let Some(output) = output.filter(|output| !output.trim().is_empty()) else {
        return lines;
    };
    lines.put("      it said:");
    lines.block(&indented(output));
    lines
}

/// Each line of the output, indented to sit under its finding.
fn indented(output: &str) -> String {
    output.lines().fold(String::new(), |mut block, line| {
        block.push_str("        ");
        block.push_str(line);
        block.push('\n');
        block
    })
}

/// The problem's meaning and remedies, indented under a finding.
pub(super) fn remedies(problem: &Problem) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("      {}", problem.meaning));
    // Which of the three kinds this is, said where it changes what the operator can do.
    // The other two already say so in their own remedies — one names where to go and the
    // other offers the bundle — and only this one has an answer they would not guess at.
    if mendable(problem.state) {
        lines.put("      lemonfiber can put this one right for you".to_owned());
        lines.put(format!("        {ASK_FOR_REPAIRS}"));
    }
    for remedy in &problem.remedies {
        lines.remedy(remedy, "      ");
    }
    lines
}

/// The one-line verdict a diagnosis amounts to.
pub(super) fn overall(overall: Overall) -> &'static str {
    match overall {
        Overall::Healthy => "healthy — everything checked passed",
        Overall::Degraded => "degraded — working, with warnings",
        Overall::Broken => "broken — something needs attention",
        Overall::Unknown => "unknown — health could not be established",
    }
}

#[cfg(test)]
mod tests;
