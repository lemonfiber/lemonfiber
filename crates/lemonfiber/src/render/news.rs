//! What a surface can mark as new, on a terminal.
//!
//! Three short lists, each newest first and each led by what names its items: the
//! version, the request's number, the check. Those are what a reader matches against
//! what they last saw, so they are the first thing on each line, and the words follow.
//!
//! A kind that could not be read says so in place of its list. An empty list under a
//! heading reads as nothing being there, which is a different answer.

use std::time::{Duration, UNIX_EPOCH};

use lemonfiber_core::instant;
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
        Some(title) => format!("#{}   {title}, asked for by {}", request.number, request.by),
        None => format!("#{}   asked for by {}", request.number, request.by),
    }
}

fn problem(problem: &NewsProblem) -> String {
    format!(
        "{}   since {}   {}",
        problem.check,
        when(&problem.onset),
        problem.summary
    )
}

/// An onset as a date and a time in UTC, or as the stack stamped it where it is not a
/// number of seconds.
fn when(onset: &str) -> String {
    onset
        .parse::<u64>()
        .ok()
        .and_then(|seconds| instant::written(UNIX_EPOCH + Duration::from_secs(seconds)))
        .map_or_else(|| onset.to_owned(), |written| format!("{written} UTC"))
}

#[cfg(test)]
mod tests;
