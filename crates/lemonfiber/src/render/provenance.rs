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
mod tests;
