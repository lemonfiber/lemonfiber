use lemonfiber_core::model::{ProvenanceReport, ServiceProvenance};

use super::comes_from;

/// One service, as the stack would declare it.
fn service(id: &str, license: &str) -> ServiceProvenance {
    ServiceProvenance {
        id: id.to_owned(),
        name: id.to_owned(),
        license: license.to_owned(),
        upstream: format!("https://github.com/{id}/{id}"),
        image: format!("lscr.io/linuxserver/{id}"),
        pinned: "4.0.15".to_owned(),
    }
}

/// The three facts the entry exists for, each on the screen and each labelled —
/// an identifier nobody can tell from a version is not something to check.
#[test]
fn a_service_is_named_with_its_licence_its_project_and_what_is_actually_run() {
    let said = comes_from(&ProvenanceReport {
        services: vec![service("sonarr", "GPL-3.0-only")],
    })
    .text();

    assert!(said.contains("sonarr — sonarr"), "{said}");
    assert!(said.contains("licence  GPL-3.0-only"), "{said}");
    assert!(
        said.contains("project  https://github.com/sonarr/sonarr"),
        "{said}"
    );
    assert!(
        said.contains("pinned   lscr.io/linuxserver/sonarr:4.0.15"),
        "{said}"
    );
}

/// The question underneath the listing, answered once at the bottom: the set of
/// licences rather than a repetition of the lines above it.
#[test]
fn the_listing_closes_on_the_set_of_licences_and_says_what_holds_it() {
    let said = comes_from(&ProvenanceReport {
        services: vec![
            service("sonarr", "GPL-3.0-only"),
            service("radarr", "GPL-3.0-only"),
            service("gluetun", "MIT"),
        ],
    })
    .text();

    assert!(
        said.contains("3 services, under 2 licences: GPL-3.0-only, MIT."),
        "{said}"
    );
    assert!(said.contains("OSI-approved"), "{said}");
}

/// One of each, because a count that reads "1 services" is a report nobody wrote
/// and everybody notices.
#[test]
fn one_service_under_one_licence_is_said_in_the_singular() {
    let said = comes_from(&ProvenanceReport {
        services: vec![service("sonarr", "GPL-3.0-only")],
    })
    .text();

    assert!(said.contains("1 service, under 1 licence:"), "{said}");
}

/// Through the printer rather than by calling this module, because what the
/// terminal draws is what the printer chose for the outcome — and an arm nothing
/// reaches renders nowhere however good the renderer under it is.
#[test]
fn the_printer_reaches_this_renderer_for_this_outcome() {
    let report = ProvenanceReport {
        services: vec![service("sonarr", "GPL-3.0-only")],
    };
    let drawn = crate::render::shaped(&lemonfiber_core::app::Outcome::Provenance(report)).text();

    assert!(
        drawn.contains("Where each service in this stack"),
        "{drawn}"
    );
    assert!(drawn.contains("GPL-3.0-only"), "{drawn}");
}

/// A stack mid-edit says it declares none, rather than leaving a heading over
/// nothing and reading as a command that fell over.
#[test]
fn a_stack_with_no_services_says_so_rather_than_showing_a_bare_heading() {
    let said = comes_from(&ProvenanceReport {
        services: Vec::new(),
    })
    .text();

    assert!(said.contains("declares no services"), "{said}");
}
