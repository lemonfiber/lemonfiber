//! What a surface can mark as new, on a terminal.
//!
//! Three short lists, each newest first and each led by what names its items: the
//! version, the request's reference, the check. Those are what a reader matches against
//! what they last saw, so they are the first thing on each line, and the words follow.
//!
//! A kind that could not be read says so in place of its list. An empty list under a
//! heading reads as nothing being there, which is a different answer.

use lemonfiber_core::instant::{self, Instant};
use lemonfiber_core::news::{News, NewsKind, NewsProblem, NewsRequest, NewsUpdate};

use super::Lines;

/// Said in place of a list whose kind could not be read.
const UNREAD: &str = "  could not be read just now";

/// Said in place of a list that was read and holds nothing.
const NOTHING: &str = "  none";

/// What a surface can mark as new on this stack, newest first within each kind.
pub(super) fn news(report: &News) -> Lines {
    let mut lines = Lines::default();
    lines.put("Releases, newest first:");
    listed(
        &mut lines,
        report,
        NewsKind::Updates,
        report.updates.iter().map(update).collect(),
    );
    lines.spaced("Requests, newest first:");
    listed(
        &mut lines,
        report,
        NewsKind::Requests,
        report.requests.iter().map(request).collect(),
    );
    lines.spaced("Problems, newest first:");
    listed(
        &mut lines,
        report,
        NewsKind::Problems,
        report.problems.iter().map(problem).collect(),
    );
    lines
}

/// One kind's items, or what stands in for them.
fn listed(lines: &mut Lines, report: &News, kind: NewsKind, items: Vec<String>) {
    if report.unread.contains(&kind) {
        lines.put(UNREAD);
        return;
    }
    if items.is_empty() {
        lines.put(NOTHING);
    }
    for item in items {
        lines.put(format!("  {item}"));
    }
}

fn update(update: &NewsUpdate) -> String {
    match &update.delivers {
        Some(delivers) => format!("{}   {delivers}", update.version),
        None => update.version.clone(),
    }
}

fn request(request: &NewsRequest) -> String {
    match &request.title {
        Some(title) => format!(
            "#{}   {title}, asked for by {}",
            request.request, request.by
        ),
        None => format!("#{}   asked for by {}", request.request, request.by),
    }
}

fn problem(problem: &NewsProblem) -> String {
    format!(
        "{}   since {}   {}",
        problem.check,
        when(problem.onset),
        problem.summary
    )
}

/// An instant as a date and a time in UTC, or as its seconds where no calendar holds it.
pub(super) fn when(at: Instant) -> String {
    instant::written(at.time()).map_or_else(|| at.to_string(), |written| format!("{written} UTC"))
}

/// A moment in seconds since the Unix epoch, as a date and a time in UTC.
pub(super) fn moment(seconds: u64) -> String {
    when(Instant::from_seconds(seconds))
}

#[cfg(test)]
mod tests;
