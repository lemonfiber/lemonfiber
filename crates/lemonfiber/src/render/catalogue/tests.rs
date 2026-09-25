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
