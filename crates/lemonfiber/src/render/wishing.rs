//! A member's search for a title, and what asking for one came to, printed.

use lemonfiber_core::model::{AskableReport, AskedReport, Stands};
use lemonfiber_core::ports::media::Kind;

use super::Lines;

/// One page of what a search found, a title a line, and where the next page begins.
pub(super) fn askable(report: &AskableReport) -> Lines {
    let mut lines = Lines::default();
    let kinds: Vec<&str> = report
        .kinds
        .iter()
        .map(|kind| Kind::from(*kind).noun())
        .collect();
    lines.put(format!(
        "{} — \"{}\", among every {}",
        report.member,
        report.term,
        kinds.join(" and every ")
    ));
    for title in &report.titles {
        let year = title
            .year
            .map_or_else(String::new, |year| format!(" ({year})"));
        lines.put(format!(
            "  {}{year} · {} · {} · {}",
            title.title,
            Kind::from(title.kind).noun(),
            stands(title.stands),
            title.id
        ));
    }
    if report.titles.is_empty() {
        lines.put("  Nothing on this page they may ask for.");
    }
    lines.spaced(report.approval.clone());
    if let Some(sentence) = report.asking.sentence() {
        lines.put(format!("Their allowance: {sentence}."));
    }
    if let Some(next) = &report.next {
        lines.spaced(format!("More: --after {next}"));
    }
    lines
}

/// What asking came to, in the household's own sentence.
pub(super) fn asked(report: &AskedReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("{} — {}", report.member, report.said));
    lines
}

/// Where a title stands, as a line names it.
const fn stands(stands: Stands) -> &'static str {
    match stands {
        Stands::Here => "here",
        Stands::PartlyHere => "partly here",
        Stands::OnItsWay => "on its way",
        Stands::Askable => "may be asked for",
    }
}

#[cfg(test)]
mod tests;
