//! Where each service comes from, on a terminal.
//!
//! The licence leads each entry, because it is the sentence somebody came here to
//! read: the claim this stack makes about itself is that everything in it is open
//! source, and the identifier is the part of the entry that either holds that claim
//! up or does not. The project follows, because it is where somebody goes to check
//! the identifier against the thing it describes, and the exact image is last,
//! because it is what a check against a project has to be a check *of* — a licence
//! read off a repository's front page says nothing about the version being run.
//!
//! The image and the tag are printed as one reference rather than as two fields. They
//! are carried apart so a caller can compare versions without parsing them out of a
//! string, and joined here because a version on its own names nothing anybody can
//! fetch, and fetching it is what verifying comes to.
//!
//! It closes on the licences rather than the services. Nineteen entries is more than
//! anybody holds at once, and the question underneath the listing — *is all of this
//! open source* — is answered by the set of identifiers rather than by any one line,
//! so the set is stated once and the check that keeps it true is named beside it.

use std::collections::BTreeSet;

use lemonfiber_core::model::{ProvenanceReport, ServiceProvenance};
use lemonfiber_core::plural::s;

use super::Lines;

/// What the listing is, said before it.
const HEADING: &str = "Where each service in this stack comes from:";

/// Why the licences below can be read as checked rather than as claimed.
///
/// Worth a sentence rather than left for somebody to discover, because it is the
/// difference between a list of strings the stack wrote down and a list nothing else
/// would have let it run with.
const CHECKED: &str =
    "Every one of these is an OSI-approved licence, and lemonfiber refuses to read a \
     stack declaring anything else — so this is what is enforced rather than what is \
     claimed.";

/// Where every service in this stack comes from, and under what licence.
pub(crate) fn comes_from(report: &ProvenanceReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(HEADING);
    for service in &report.services {
        lines.extend(entry(service));
    }
    lines.extend(licences(&report.services));
    lines
}

/// One service: what it is, what it is published under, and what is actually run.
fn entry(service: &ServiceProvenance) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(format!("  {} — {}", service.id, service.name));
    lines.put(format!("    licence  {}", service.license));
    lines.put(format!("    project  {}", service.upstream));
    lines.put(format!("    pinned   {}:{}", service.image, service.pinned));
    lines
}

/// What the whole listing comes to: how many services, under which licences.
///
/// A stack declaring none is possible — somebody's own, mid-edit — and it is said as
/// a listing with nothing in it rather than left as a heading over empty space, since
/// a report that stops after its own title reads as one that failed.
fn licences(services: &[ServiceProvenance]) -> Lines {
    let mut lines = Lines::default();
    if services.is_empty() {
        lines.spaced("This stack declares no services.");
        return lines;
    }
    let named: BTreeSet<&str> = services
        .iter()
        .map(|service| service.license.as_str())
        .collect();
    lines.spaced(format!(
        "{} service{}, under {} licence{}: {}.",
        services.len(),
        s(services.len()),
        named.len(),
        s(named.len()),
        named.into_iter().collect::<Vec<&str>>().join(", ")
    ));
    lines.put(CHECKED);
    lines
}

#[cfg(test)]
mod tests {
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
        let drawn =
            crate::render::shaped(&lemonfiber_core::app::Outcome::Provenance(report)).text();

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
}
