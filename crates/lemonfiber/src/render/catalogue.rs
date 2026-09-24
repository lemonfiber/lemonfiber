//! What each service is for, on a terminal.
//!
//! The description leads each entry, because it is the sentence somebody came here to
//! read: a stack of nineteen names conveys nothing, and this is the line that turns a
//! name into something the person running it recognises. What going without costs
//! follows, because that is what converts an inventory into a judgement — knowing that
//! Bazarr finds subtitles says nothing about whether its being down is worth getting
//! up for. How much it matters closes the entry, as the one word that ranks the loss
//! against the eighteen other losses on the same screen.
//!
//! The removals go last and under a heading of their own. They answer a question the
//! listing above raises rather than one it answers — an operator reads down nineteen
//! services looking for the one they remember, and the useful thing to tell them at
//! the bottom is where it went. Told there rather than in a document, because somebody
//! looking for a service that is no longer here has no reason to know a document about
//! it exists.

use lemonfiber_core::docker::Criticality;
use lemonfiber_core::model::{CatalogueReport, CataloguedService, RemovedService};
use lemonfiber_core::plural::s;

use super::Lines;

/// What the listing is, said before it.
const HEADING: &str = "What each service in this stack is for:";

/// What the removals are, said before them.
const WENT: &str = "Services this stack used to carry:";

/// What is said of a removal that named no replacement.
///
/// Said rather than left blank, because a blank reads as a record somebody did not
/// finish. Most things that go are not replaced, and that is the answer.
const NOTHING_TOOK_OVER: &str = "nothing took over what it did";

/// What each service is for, and what became of the ones that went.
pub(crate) fn holds(report: &CatalogueReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(HEADING);
    if report.services.is_empty() {
        lines.put("  This stack declares no services.");
    }
    for service in &report.services {
        lines.extend(entry(service));
    }
    lines.extend(counted(report));
    lines.extend(removals(&report.removed));
    lines
}

/// One service: what it is called, what it does, what its absence costs, and how much
/// that matters.
fn entry(service: &CataloguedService) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(format!("  {} — {}", service.id, service.name));
    lines.put(format!("    does     {}", service.describes));
    lines.put(format!("    without  {}", service.without_it));
    lines.put(format!("    matters  {}", matters(service.criticality)));
    lines
}

/// How much a service's absence matters, as a word with its meaning beside it.
///
/// The word alone is a rank nobody was told the scale of. `critical` in particular has
/// to say what it means here, because it means something narrower and sharper than the
/// sense an operator brings to it: not *the stack stops* but *the consequence leaves
/// this machine*.
const fn matters(criticality: Criticality) -> &'static str {
    match criticality {
        Criticality::Critical => "critical — its failure has consequences beyond this machine",
        Criticality::Core => "core — the stack cannot do its job without it",
        Criticality::Important => "important — a significant capability is lost",
        Criticality::Enhancing => "enhancing — quality of life",
        Criticality::Optional => "optional — off unless you ask for it",
    }
}

/// What the listing comes to: how many services, and how many of them are worth
/// getting out of bed for.
///
/// The count of the two heaviest ranks rather than of all five, because that is the
/// number somebody is actually carrying away — nineteen services is more than anybody
/// holds at once, and *which of these matter* is the question underneath the list.
fn counted(report: &CatalogueReport) -> Lines {
    let mut lines = Lines::default();
    if report.services.is_empty() {
        return lines;
    }
    let weighty = report
        .services
        .iter()
        .filter(|service| {
            matches!(
                service.criticality,
                Criticality::Critical | Criticality::Core
            )
        })
        .count();
    lines.spaced(format!(
        "{} service{}, of which {weighty} the stack cannot do its job without.",
        report.services.len(),
        s(report.services.len())
    ));
    lines
}

/// What the stack used to carry, where it has recorded anything.
///
/// Silent where nothing has gone, rather than saying so. A heading over an empty list
/// is a section about a thing that never happened, and every stack starts there.
fn removals(removed: &[RemovedService]) -> Lines {
    let mut lines = Lines::default();
    if removed.is_empty() {
        return lines;
    }
    lines.spaced(WENT);
    for gone in removed {
        lines.spaced(format!("  {} — went in {}", gone.id, gone.removed_in));
        lines.put(format!("    why      {}", gone.reason));
        lines.put(format!("    instead  {}", instead(gone)));
    }
    lines
}

/// What took a removed service's place, or that nothing did.
fn instead(gone: &RemovedService) -> &str {
    gone.replaced_by.as_deref().unwrap_or(NOTHING_TOOK_OVER)
}

#[cfg(test)]
mod tests;
