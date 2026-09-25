//! What a proposed change comes to on this machine, said before it is written.
//!
//! One of the renderers, its own file so each answer's shape is read on its own.
//! Every one of them builds lines and hands them back; the printer is at the edge.
//!
//! The order is the order somebody weighing the change reads it in: where the
//! library would land, what is still coming down, what was edited underneath, then
//! what the change opens, stops and keeps. The verdict and its refusal are said by
//! the settings renderer above, once, for every stance alike.

use lemonfiber_core::reconfigure::Findings;

use super::Lines;

/// What was found about this change, or nothing where nothing was.
pub(super) fn found(findings: &Findings) -> Lines {
    let mut lines = Lines::default();
    if !findings.any() {
        return lines;
    }
    lines.extend(library(findings));
    lines.extend(active(findings));
    lines.extend(edited(findings));
    lines.extend(opened(findings));
    lines.extend(closed(findings));
    lines
}

/// The library paths a move would land on, and where each of them would land.
fn library(findings: &Findings) -> Lines {
    let mut lines = Lines::default();
    if findings.library.is_empty() {
        return lines;
    }
    lines.spaced("Where the services file now:");
    for path in &findings.library {
        let mark = if path.carried { "kept" } else { "lost" };
        lines.put(format!("  {mark}  {} — {}", path.path, path.because));
    }
    lines
}

/// What is still coming down that this change would interrupt.
fn active(findings: &Findings) -> Lines {
    let mut lines = Lines::default();
    if findings.active.is_empty() {
        return lines;
    }
    lines.spaced("Still coming down:");
    for download in &findings.active {
        lines.put(format!(
            "  {}% {} ({})",
            download.progress, download.name, download.protocol
        ));
    }
    lines
}

/// The hand-edit found under this setting, both sides of it.
fn edited(findings: &Findings) -> Lines {
    let mut lines = Lines::default();
    let Some(edit) = &findings.edited else {
        return lines;
    };
    lines.spaced("Changed outside lemonfiber:");
    lines.put(format!("  lemonfiber wrote  {}", edit.wrote));
    lines.put(format!("  the file holds    {}", edit.found));
    lines
}

/// What this change newly asks the operator for.
fn opened(findings: &Findings) -> Lines {
    let mut lines = Lines::default();
    if findings.opens.is_empty() {
        return lines;
    }
    lines.spaced("This opens:");
    for open in &findings.opens {
        lines.put(format!("  {} — {}", open.what, open.because));
    }
    lines
}

/// What this change stops, and what it leaves exactly as it is.
fn closed(findings: &Findings) -> Lines {
    let mut lines = Lines::default();
    if !findings.stops.is_empty() {
        lines.spaced(format!("This stops: {}", findings.stops.join(", ")));
    }
    if !findings.keeps.is_empty() {
        lines.spaced("This keeps:");
        for kept in &findings.keeps {
            lines.put(format!("  {kept}"));
        }
    }
    lines
}

#[cfg(test)]
mod tests;
