//! The download queue, as `lemonfiber queue` says it.

use lemonfiber_core::instant::MINUTE;
use lemonfiber_core::model::{
    Act, ActOutcome, ActTaken, HealRun, HealingSwitch, QueueHealth, Wedge, Wedged,
};

use super::news::when;
use super::Lines;

/// Whether healing acts, what is wedged and why, what could not be reached, and what the
/// newest runs did.
pub(super) fn queue(report: &QueueHealth) -> Lines {
    let mut lines = Lines::default();
    lines.put(match report.healing {
        HealingSwitch::On => format!(
            "Healing is on: a download flagged for one reason on {} passes in a row, past its \
             first {} minutes, is dropped, its release blocklisted, and searched for again.",
            report.strikes_needed,
            report.grace_s / MINUTE
        ),
        HealingSwitch::Off => {
            "Healing is off: the queue is classified, and nothing is acted on.".to_owned()
        }
    });
    if report.rehearsed {
        lines.put("This run only said what it would do.");
    }
    match report.classified_at {
        Some(at) => lines.put(format!("Last looked at {}.", when(&at.to_string()))),
        None => lines.put("The queue has not been looked at yet."),
    }
    if report.incomplete {
        lines.put("Something could not be read, so this list may be short.");
    }
    if let Some(mismatch) = &report.mismatch {
        lines.spaced(format!("Nothing is acted on where {mismatch}."));
    }
    if !report.unreachable.is_empty() {
        lines.spaced("Could not be reached or verified:");
        for one in &report.unreachable {
            lines.put(format!("  {}: {}", one.service, one.why));
        }
    }
    if report.items.is_empty() {
        if report.classified_at.is_some() {
            lines.spaced("Nothing in the queue is wedged.");
        }
    } else {
        lines.spaced("Wedged:");
        for item in &report.items {
            wedged(&mut lines, item, report.strikes_needed);
        }
    }
    if !report.runs.is_empty() {
        lines.spaced("Newest runs:");
        for run in &report.runs {
            ran(&mut lines, run);
        }
    }
    lines
}

/// One wedged download: what it is, why, its strikes, and what the next run does to it.
fn wedged(lines: &mut Lines, item: &Wedged, needed: u32) {
    lines.put(format!(
        "  {} at {}: {}, {}; strike {} of {needed}",
        item.title.as_deref().unwrap_or(&item.release),
        item.service,
        class(item.class),
        item.why,
        item.strikes
    ));
    if let Some(until) = item.in_grace_until {
        lines.put(format!("    left alone until {}", when(&until.to_string())));
    }
    match item.next.as_deref() {
        Some([]) => lines.put("    next run: nothing, as no curator holds it by a queue entry"),
        Some(next) => lines.put(format!(
            "    next run: {}",
            next.iter()
                .map(|act| act_word(*act))
                .collect::<Vec<_>>()
                .join(", ")
        )),
        None => {}
    }
}

/// One run: when, whether it was a rehearsal, and each item's three acts.
fn ran(lines: &mut Lines, run: &HealRun) {
    lines.put(format!(
        "  {}{}",
        when(&run.at.to_string()),
        if run.rehearsed { ", rehearsed" } else { "" }
    ));
    for item in &run.items {
        lines.put(format!(
            "    {} at {}, because {}",
            item.title.as_deref().unwrap_or(&item.release),
            item.service,
            item.why
        ));
        lines.put(format!(
            "      {}",
            item.acts.iter().map(taken).collect::<Vec<_>>().join("; ")
        ));
    }
}

/// One act as it came out.
fn taken(taken: &ActTaken) -> String {
    let act = act_word(taken.act);
    let outcome = match (taken.outcome, taken.instead) {
        (ActOutcome::Done, _) => "done".to_owned(),
        (ActOutcome::Would, _) => "would".to_owned(),
        (ActOutcome::FellBack, Some(instead)) => format!("not done, {} instead", act_word(instead)),
        (ActOutcome::Refused | ActOutcome::FellBack, _) => "not done".to_owned(),
    };
    match (&taken.why, taken.outcome) {
        (Some(why), ActOutcome::Refused | ActOutcome::FellBack) => {
            format!("{act}: {outcome}, {why}")
        }
        _ => format!("{act}: {outcome}"),
    }
}

/// An act, in words.
const fn act_word(act: Act) -> &'static str {
    match act {
        Act::Remove => "drop",
        Act::Blocklist => "blocklist",
        Act::Search => "search again",
    }
}

/// Why a download is wedged, in a word or two.
const fn class(class: Wedge) -> &'static str {
    match class {
        Wedge::Stalled => "stalled",
        Wedge::Slow => "slow",
        Wedge::Failed => "failed",
        Wedge::StuckOnMetadata => "stuck on metadata",
        Wedge::Orphaned => "orphaned",
    }
}

#[cfg(test)]
mod tests;
