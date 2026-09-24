//! What this stack wires to what, on a terminal.
//!
//! The capability leads each line rather than the service, because that is the thing
//! the link is about: what this asks for is the durable fact, and which service
//! answers it today is the part that can change. A listing that led with the service
//! would read as the old wiring with extra words.
//!
//! A by-name link is shown as one, with its reason. That is the whole point of the
//! exception being written down: an operator reading twenty links needs to see at a
//! glance which one is deliberate, and why, rather than reading it as the one nobody
//! got round to converting.
//!
//! What nothing fills is repeated at the bottom, naming what asked. A stack with one
//! broken link among twenty working ones is a stack whose problem is a line in a
//! list, and a reader who had to spot it is the reason nobody would.

use lemonfiber_core::model::{SubstitutionReport, WiringReport};
use lemonfiber_core::plural::s;
use lemonfiber_core::wiring::{Reaches, Settled, Unfilled, Whose, Wired};

use super::Lines;

/// What the listing is, said before it.
const HEADING: &str = "What this stack wires to what:";

/// What the unfilled asks are, said before them.
const NOTHING_FILLS: &str = "Asked for, and nothing fills it:";

/// Every link, and what nothing fills.
pub(crate) fn wired(report: &WiringReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(HEADING);
    if report.wired.is_empty() {
        lines.put("  This stack declares no wiring.");
    }
    for link in &report.wired {
        lines.extend(entry(link));
    }
    lines.extend(counted(report));
    lines.extend(missing(&report.unfilled));
    lines
}

/// One link: who asks, what for, and what answers.
fn entry(link: &Wired) -> Lines {
    let mut lines = Lines::default();
    match &link.reaches {
        Reaches::Asked {
            capability,
            services,
            settled,
            origins,
        } => {
            lines.spaced(format!("  {} asks for {capability}", link.by));
            lines.put(format!("    reaches  {}", reached(services, origins)));
            for said in settling(settled) {
                lines.put(format!("    {said}"));
            }
        }
        Reaches::ByName { service, why } => {
            lines.spaced(format!("  {} is wired to {service} by name", link.by));
            lines.put(format!("    why      {why}"));
        }
    }
    lines
}

/// What an ask reaches, or that it reaches nothing, each beside where it came from.
///
/// Beside the name rather than in a column of its own, because reading the one and
/// reading the other has to be the same act: an operator who never thought to ask
/// whether a plugin is involved is the one this is for.
fn reached(
    services: &[String],
    origins: &std::collections::BTreeMap<String, lemonfiber_core::origin::Origin>,
) -> String {
    match services {
        [] => "nothing".to_owned(),
        named => named
            .iter()
            .map(|service| format!("{service} ({})", from(origins.get(service))))
            .collect::<Vec<String>>()
            .join(", "),
    }
}

/// Where one service came from, in the words a listing uses.
///
/// A service the answer names no origin for is said to be of unknown origin rather
/// than left bare, which would read as this build's own.
fn from(origin: Option<&lemonfiber_core::origin::Origin>) -> String {
    match origin {
        Some(lemonfiber_core::origin::Origin::Plugin { named }) => format!("plugin {named}"),
        // The vocabulary's own word for every other origin — `bundled` for the stack's —
        // so this is one more reader of it rather than a second list of the words.
        Some(other) => other.as_str().to_owned(),
        None => "unknown".to_owned(),
    }
}

/// How the ask was settled, where that is worth a line of its own.
///
/// Nothing for the ordinary case. One claimant and nothing to settle is what most
/// links are, and a line saying so on every one of them would bury the three that
/// somebody has to act on.
fn settling(settled: &Settled) -> Vec<String> {
    match settled {
        Settled::Outright => Vec::new(),
        Settled::Each => vec!["reaching  every service that fills it".to_owned()],
        Settled::Contested { claimants } => vec![
            format!(
                "contested {} claim it: {}",
                claimants.len(),
                claimants.join(", ")
            ),
            "          choose one with `lemonfiber wiring fill`".to_owned(),
        ],
        Settled::Chosen { whose, why, over } => {
            let by = match whose {
                Whose::Stack => "the stack's choice",
                Whose::Operator => "your choice",
            };
            let mut said = vec![format!("chosen    {by}, over {}", over.join(", "))];
            said.extend(why.iter().map(|reason| format!("          {reason}")));
            said
        }
        Settled::Unfilled => vec!["unfilled  nothing in this stack provides it".to_owned()],
    }
}

/// How many links there are, and how many of them are by name.
fn counted(report: &WiringReport) -> Lines {
    let mut lines = Lines::default();
    if report.wired.is_empty() {
        return lines;
    }
    let by_name = report
        .wired
        .iter()
        .filter(|link| matches!(link.reaches, Reaches::ByName { .. }))
        .count();
    let total = report.wired.len();
    lines.spaced(format!(
        "{total} link{}, {} of them by name.",
        s(total),
        by_name
    ));
    lines
}

/// Every ask nothing fills, each naming what asked for it.
fn missing(unfilled: &[Unfilled]) -> Lines {
    let mut lines = Lines::default();
    if unfilled.is_empty() {
        return lines;
    }
    lines.spaced(NOTHING_FILLS);
    for one in unfilled {
        lines.put(format!("  {} — asked for by {}", one.capability, one.by));
    }
    lines
}

/// What a substitution did, or would do.
pub(crate) fn substituted(report: &SubstitutionReport) -> Lines {
    let made = &report.substitution;
    let mut lines = Lines::default();
    let verb = if report.applied {
        "now fills"
    } else {
        "would fill"
    };
    lines.put(format!("{} {verb} {}.", made.now, made.capability));
    if let Some(was) = &made.was {
        lines.put(format!("  was      {was}"));
    }
    lines.put(format!("  asked by {}", made.asked_by.join(", ")));
    if !report.applied {
        lines.spaced("Nothing was written. Run it without --dry-run to make the change.");
    }
    lines.extend(cost(&made.leaves_unfilled, report.applied));
    lines
}

/// What the change leaves with nothing filling it, said before it is agreed to.
fn cost(leaves: &[Unfilled], applied: bool) -> Lines {
    let mut lines = Lines::default();
    if leaves.is_empty() {
        return lines;
    }
    let said = if applied {
        "This left nothing filling:"
    } else {
        "This would leave nothing filling:"
    };
    lines.spaced(said);
    for one in leaves {
        lines.put(format!("  {} — asked for by {}", one.capability, one.by));
    }
    lines
}

#[cfg(test)]
mod tests;
