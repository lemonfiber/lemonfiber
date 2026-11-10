//! A member's search for a title, and what asking for one came to, printed.

use lemonfiber_core::model::{
    AskableKindsReport, AskableReport, AskedReport, Stands, TitleKind, TitleUnit,
};

use super::Lines;

/// One page of what a search found, a title a line, and where the next page begins.
pub(super) fn askable(report: &AskableReport) -> Lines {
    let mut lines = Lines::default();
    let kinds: Vec<&str> = report.kinds.iter().map(|kind| kind.noun()).collect();
    lines.put(format!(
        "{} — \"{}\", among every {}",
        report.member,
        report.term,
        kinds.join(" and every ")
    ));
    for title in &report.titles {
        let by = title
            .by
            .as_ref()
            .map_or_else(String::new, |by| format!(" by {by}"));
        let year = title
            .year
            .map_or_else(String::new, |year| format!(" ({year})"));
        lines.put(format!(
            "  {}{by}{year} · {} · {} · {}",
            title.title,
            title.unit.map_or(title.kind.noun(), TitleUnit::noun),
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

/// Which kinds a member may ask for now, a kind a line with what it is asked for as.
pub(super) fn kinds(report: &AskableKindsReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("{} may ask for", report.member));
    for one in &report.kinds {
        let units: Vec<&str> = one.units.iter().map(|unit| unit.noun()).collect();
        if units.is_empty() {
            lines.put(format!("  {}", called(one.kind)));
        } else {
            lines.put(format!("  {}: {}", called(one.kind), units.join(", ")));
        }
    }
    if report.kinds.is_empty() {
        lines.put("  Nothing right now.");
    }
    lines
}

/// What asking came to, in the household's own sentence.
pub(super) fn asked(report: &AskedReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("{} — {}", report.member, report.said));
    if let Some(request) = &report.request {
        lines.put(format!("On the household list as {request}."));
    }
    lines
}

/// What a household calls a kind, all of it.
const fn called(kind: TitleKind) -> &'static str {
    match kind {
        TitleKind::Tv => "series",
        TitleKind::Movies => "films",
        TitleKind::Music => "music",
        TitleKind::Books => "books",
    }
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
