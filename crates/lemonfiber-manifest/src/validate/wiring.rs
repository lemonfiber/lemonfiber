//! Checking the links a manifest declares between its own services.
//!
//! Apart from the rules about a service because the subject is different: a service
//! is checked against itself, and a link is checked against the two services it
//! joins and against what they say they can do. The one rule that spans both halves
//! is here too, because it is about a link — an ordering edge names a service, so it
//! is a by-name link and has to be declared as one.

use std::collections::{BTreeMap, BTreeSet};

use super::{is_core_name, Violation};
use crate::{Manifest, Wiring};

/// Every link between two services, and the rule that holds the two halves together.
pub(super) fn check(manifest: &Manifest, found: &mut Vec<Violation>) {
    let declared: BTreeSet<&str> = manifest
        .services
        .iter()
        .map(|service| service.id.as_str())
        .collect();
    let provides: BTreeMap<&str, &[String]> = manifest
        .services
        .iter()
        .map(|service| (service.id.as_str(), service.provides.as_slice()))
        .collect();

    let mut seen: BTreeSet<(&str, &str, &str)> = BTreeSet::new();
    for wiring in &manifest.wirings {
        let faults = ends(wiring, &declared)
            .into_iter()
            .chain(one_end(wiring))
            .chain(asked(wiring, &provides))
            .chain(named(wiring))
            .chain(once(wiring, &mut seen));

        let location = format!("wiring by {}", wiring.by);
        found.extend(faults.map(|message| Violation {
            location: location.clone(),
            message,
        }));
    }
    shown_as_by_name(manifest, found);
}

/// Every ordering edge is also declared as a by-name link.
///
/// An ordering edge names a service and the engine acts on it, so it is a by-name
/// link by any reading. One that is not declared here would be the exception without
/// having had to say it was one, which is the whole of what the table is for.
fn shown_as_by_name(manifest: &Manifest, found: &mut Vec<Violation>) {
    let shown: BTreeSet<(&str, &str)> = manifest
        .wirings
        .iter()
        .filter_map(|wiring| Some((wiring.by.as_str(), wiring.to.as_deref()?)))
        .collect();
    for service in &manifest.services {
        for on in &service.depends_on {
            if shown.contains(&(service.id.as_str(), on.as_str())) {
                continue;
            }
            found.push(Violation {
                location: format!("service {}", service.id),
                message: format!(
                    "depends_on {on} and no wiring shows it as by name; an ordering edge \
                     names a service, so it is the exception and has to say so"
                ),
            });
        }
    }
}

/// Every end a link names is a service this stack declares, and is not its own end.
fn ends(wiring: &Wiring, declared: &BTreeSet<&str>) -> Vec<String> {
    let unknown = |field: &str, named: &str| {
        (!declared.contains(named))
            .then(|| format!("{field} names {named}, which is not a service"))
    };
    let itself = |field: &str, named: &str| {
        (named == wiring.by)
            .then(|| format!("{field} names the service it runs from; nothing wires to itself"))
    };
    unknown("by", &wiring.by)
        .into_iter()
        .chain(
            wiring
                .to
                .iter()
                .flat_map(|to| unknown("to", to).into_iter().chain(itself("to", to))),
        )
        .chain(wiring.filled_by.iter().flat_map(|filler| {
            unknown("filled_by", filler)
                .into_iter()
                .chain(itself("filled_by", filler))
        }))
        .collect()
}

/// A link asks for a capability or names a service, and carries exactly one of them.
///
/// Both is an ask whose answer was decided in advance, which is a name written the
/// long way. Neither is a link to nowhere.
fn one_end(wiring: &Wiring) -> Vec<String> {
    match (wiring.asks.as_deref(), wiring.to.as_deref()) {
        (Some(_), Some(_)) => vec![
            "asks for a capability and names a service; a wiring does one or the other".to_owned(),
        ],
        (None, None) => vec!["neither asks for a capability nor names a service".to_owned()],
        _ => Vec::new(),
    }
}

/// What an ask has to get right, including the claimant it chose where it chose one.
fn asked(wiring: &Wiring, provides: &BTreeMap<&str, &[String]>) -> Vec<String> {
    let Some(asks) = wiring.asks.as_deref() else {
        return Vec::new();
    };
    let shape = (!is_core_name(asks))
        .then(|| format!("asks for {asks}, which is not a core capability name"));
    let both = (wiring.each && wiring.filled_by.is_some()).then(|| {
        "reaches every filler and also chooses one; it does both only by meaning neither".to_owned()
    });
    shape
        .into_iter()
        .chain(both)
        .chain(chosen(wiring, asks, provides))
        .collect()
}

/// What choosing a claimant has to get right: a reason, and a service that claims it.
fn chosen(wiring: &Wiring, asks: &str, provides: &BTreeMap<&str, &[String]>) -> Vec<String> {
    let Some(filler) = wiring.filled_by.as_deref() else {
        return Vec::new();
    };
    let unreasoned = said(wiring.why.as_deref())
        .is_none()
        .then(|| format!("chooses {filler} and does not say why"));
    let declares = provides
        .get(filler)
        .is_some_and(|named| named.iter().any(|one| one == asks));
    let unclaimed =
        (!declares).then(|| format!("chooses {filler} to fill {asks}, which it does not provide"));
    unreasoned.into_iter().chain(unclaimed).collect()
}

/// What a by-name link has to get right, and what only an ask may say.
fn named(wiring: &Wiring) -> Vec<String> {
    let Some(to) = wiring.to.as_deref() else {
        return Vec::new();
    };
    let unreasoned = said(wiring.why.as_deref())
        .is_none()
        .then(|| format!("names {to} and does not say why; the exception has to say it is one"));
    let asked_only = [
        ("each", wiring.each),
        ("filled_by", wiring.filled_by.is_some()),
    ]
    .into_iter()
    .filter(|(_, present)| *present)
    .map(|(field, _)| format!("carries {field}, which says something only an ask can say"));
    unreasoned.into_iter().chain(asked_only).collect()
}

/// One link per pair of ends, so a stack cannot say the same thing twice.
fn once<'a>(
    wiring: &'a Wiring,
    seen: &mut BTreeSet<(&'a str, &'a str, &'a str)>,
) -> Option<String> {
    let (kind, far) = match (wiring.asks.as_deref(), wiring.to.as_deref()) {
        (Some(asks), None) => ("asks", asks),
        (None, Some(to)) => ("to", to),
        _ => return None,
    };
    (!seen.insert((wiring.by.as_str(), kind, far))).then(|| format!("names {far} more than once"))
}

/// A reason that is a reason, rather than a field somebody filled in.
fn said(why: Option<&str>) -> Option<&str> {
    why.map(str::trim).filter(|reason| !reason.is_empty())
}

#[cfg(test)]
mod tests;
