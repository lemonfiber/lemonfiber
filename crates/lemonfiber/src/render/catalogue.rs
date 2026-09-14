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
mod tests {
    use lemonfiber_core::model::{CatalogueReport, CataloguedService, RemovedService};

    use super::{holds, Criticality};

    /// One service, as the stack would declare it.
    fn service(id: &str, criticality: Criticality) -> CataloguedService {
        CataloguedService {
            id: id.to_owned(),
            name: id.to_owned(),
            describes: format!("Does the {id} job for you"),
            without_it: format!("No {id} job gets done"),
            criticality,
        }
    }

    /// A listing of one service, for the tests about the entry itself.
    fn one(criticality: Criticality) -> CatalogueReport {
        CatalogueReport {
            services: vec![service("bazarr", criticality)],
            removed: Vec::new(),
        }
    }

    /// The three facts the entry exists for, each on the screen and each labelled —
    /// a description with no cost beside it is an inventory again.
    #[test]
    fn a_service_is_named_with_what_it_does_and_what_going_without_costs() {
        let said = holds(&one(Criticality::Enhancing)).text();

        assert!(said.contains("bazarr — bazarr"), "{said}");
        assert!(
            said.contains("does     Does the bazarr job for you"),
            "{said}"
        );
        assert!(said.contains("without  No bazarr job gets done"), "{said}");
        assert!(said.contains("matters  enhancing"), "{said}");
    }

    /// The rank carries its meaning, because `critical` here is narrower than the word
    /// an operator brings to it: the consequence leaves the machine.
    #[test]
    fn how_much_it_matters_says_what_the_word_means_rather_than_only_the_word() {
        let said = holds(&one(Criticality::Critical)).text();

        assert!(
            said.contains("critical — its failure has consequences beyond this machine"),
            "{said}"
        );
    }

    /// Every rank, because the five of them are what the whole column is: a scale with
    /// two of its five values never rendered is a scale nobody has read end to end, and
    /// the two nobody had are the two an operator meets most — the capability they lose
    /// and the thing they never turned on.
    #[test]
    fn every_rank_the_manifest_allows_says_what_it_means() {
        for (rank, meaning) in [
            (
                Criticality::Critical,
                "critical — its failure has consequences beyond this machine",
            ),
            (
                Criticality::Core,
                "core — the stack cannot do its job without it",
            ),
            (
                Criticality::Important,
                "important — a significant capability is lost",
            ),
            (Criticality::Enhancing, "enhancing — quality of life"),
            (
                Criticality::Optional,
                "optional — off unless you ask for it",
            ),
        ] {
            let said = holds(&one(rank)).text();
            assert!(said.contains(meaning), "{rank:?} reads as: {said}");
        }
    }

    /// The question underneath the listing, answered once at the bottom rather than by
    /// making somebody count nineteen lines.
    #[test]
    fn the_listing_closes_on_how_many_of_them_the_stack_cannot_work_without() {
        let said = holds(&CatalogueReport {
            services: vec![
                service("gluetun", Criticality::Critical),
                service("prowlarr", Criticality::Core),
                service("bazarr", Criticality::Enhancing),
            ],
            removed: Vec::new(),
        })
        .text();

        assert!(
            said.contains("3 services, of which 2 the stack cannot do its job without."),
            "{said}"
        );
    }

    /// A service that went is named with why and with what took over.
    #[test]
    fn a_service_that_went_is_named_with_its_reason_and_its_replacement() {
        let said = holds(&CatalogueReport {
            services: vec![service("bindery", Criticality::Enhancing)],
            removed: vec![RemovedService {
                id: "readarr".to_owned(),
                removed_in: "0.1.0".to_owned(),
                reason: "Discontinued upstream in 2025".to_owned(),
                replaced_by: Some("bindery".to_owned()),
            }],
        })
        .text();

        assert!(
            said.contains("Services this stack used to carry:"),
            "{said}"
        );
        assert!(said.contains("readarr — went in 0.1.0"), "{said}");
        assert!(
            said.contains("why      Discontinued upstream in 2025"),
            "{said}"
        );
        assert!(said.contains("instead  bindery"), "{said}");
    }

    /// Nothing took over is an answer and is written as one, because a blank reads as
    /// a record somebody did not finish.
    #[test]
    fn a_service_nothing_replaced_says_so_rather_than_leaving_the_line_blank() {
        let said = holds(&CatalogueReport {
            services: Vec::new(),
            removed: vec![RemovedService {
                id: "booksonic".to_owned(),
                removed_in: "0.2.0".to_owned(),
                reason: "Unmaintained upstream".to_owned(),
                replaced_by: None,
            }],
        })
        .text();

        assert!(
            said.contains("instead  nothing took over what it did"),
            "{said}"
        );
    }

    /// A stack that has never dropped anything gets no heading about it, because a
    /// section about a thing that never happened is where every stack starts.
    #[test]
    fn a_stack_that_has_dropped_nothing_says_nothing_about_removals() {
        let said = holds(&one(Criticality::Core)).text();

        assert!(!said.contains("used to carry"), "{said}");
    }

    /// A stack mid-edit says it declares none, rather than leaving a heading over
    /// nothing and reading as a command that fell over.
    #[test]
    fn a_stack_with_no_services_says_so_rather_than_showing_a_bare_heading() {
        let said = holds(&CatalogueReport {
            services: Vec::new(),
            removed: Vec::new(),
        })
        .text();

        assert!(said.contains("declares no services"), "{said}");
    }

    /// Every line of this listing is prose out of a file an operator can edit, and the
    /// removal's reason is the newest of it. What a terminal obeys comes out of all of
    /// it: a control sequence would clear the screen the listing is being drawn on, and
    /// a newline would let one record draw something that read as the next one.
    #[test]
    fn a_record_cannot_take_over_the_screen_or_forge_a_line() {
        let said = holds(&CatalogueReport {
            services: Vec::new(),
            removed: vec![RemovedService {
                id: "readarr".to_owned(),
                removed_in: "0.1.0".to_owned(),
                reason: "gone\u{1b}[2J\n    instead  everything is fine".to_owned(),
                replaced_by: None,
            }],
        })
        .text();

        assert!(!said.contains('\u{1b}'), "{said:?}");
        assert!(
            said.lines()
                .all(|line| !line.trim_start().starts_with("instead  everything")),
            "the forged line stayed inside the reason it was written in: {said:?}"
        );
        assert!(said.contains("instead  nothing took over"), "{said:?}");
    }

    /// Through the printer rather than by calling this module, because what the
    /// terminal draws is what the printer chose for the outcome — and an arm nothing
    /// reaches renders nowhere however good the renderer under it is.
    #[test]
    fn the_printer_reaches_this_renderer_for_this_outcome() {
        let drawn = crate::render::shaped(&lemonfiber_core::app::Outcome::Catalogue(one(
            Criticality::Core,
        )))
        .text();

        assert!(
            drawn.contains("What each service in this stack is for"),
            "{drawn}"
        );
        assert!(drawn.contains("Does the bazarr job for you"), "{drawn}");
    }
}
