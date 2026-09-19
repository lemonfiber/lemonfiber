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
mod tests {
    use crate::{validate, Date, Manifest};

    const STACK: &str = include_str!("../../../../assets/media-stack/stack.toml");

    /// After every date the stack records, so a pin bump does not fail these.
    const TODAY: Date = Date {
        year: 2026,
        month: 10,
        day: 1,
    };

    /// Everything wrong with a manifest, as one line each.
    fn messages(text: &str) -> Vec<String> {
        Manifest::from_toml(text)
            .ok()
            .map(|manifest| {
                validate(&manifest, TODAY)
                    .iter()
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Whether the stack this binary carries, edited once, says what it should.
    fn fault(from: &str, to: &str, expected: &str) {
        assert!(STACK.contains(from), "the fixture must contain {from:?}");
        let said = messages(&STACK.replacen(from, to, 1));
        assert!(
            said.iter().any(|message| message.contains(expected)),
            "expected {expected:?} among {said:?}"
        );
    }

    /// An ordering edge names a service, so it is a by-name link, so it has to be
    /// shown as one. Pointing the entry that shows it somewhere else leaves the edge
    /// unaccounted for.
    #[test]
    fn an_ordering_edge_no_wiring_shows_as_by_name_is_caught() {
        fault(
            "by = \"qbittorrent\"\nto = \"gluetun\"",
            "by = \"unpackerr\"\nto = \"sonarr\"",
            "depends_on gluetun and no wiring shows it as by name",
        );
    }

    /// A by-name link with no reason beside it reads as an oversight rather than as
    /// the exception it is, which is the whole of why it is written down.
    #[test]
    fn a_by_name_wiring_that_does_not_say_why_is_caught() {
        fault(
            "by = \"recyclarr\"\nto = \"radarr\"\nwhy = ",
            "by = \"recyclarr\"\nto = \"radarr\"\n# why = ",
            "names radarr and does not say why",
        );
    }

    /// A reason somebody filled in with spaces is not a reason.
    #[test]
    fn a_by_name_wiring_whose_reason_is_blank_is_caught() {
        fault(
            "why = \"The same, in Lidarr's terms.\"",
            "why = \"   \"",
            "names lidarr and does not say why",
        );
    }

    /// Both ends is an ask whose answer was decided in advance, which is a name
    /// written the long way.
    #[test]
    fn a_wiring_that_asks_and_names_is_caught() {
        fault(
            "by = \"seerr\"\nasks = \"identity.source\"",
            "by = \"seerr\"\nasks = \"identity.source\"\nto = \"jellyfin\"\nwhy = \"both\"",
            "a wiring does one or the other",
        );
    }

    #[test]
    fn a_wiring_with_neither_end_is_caught() {
        fault(
            "by = \"bazarr\"\nasks = \"library.curate\"\neach = true",
            "by = \"bazarr\"",
            "neither asks for a capability nor names a service",
        );
    }

    #[test]
    fn a_wiring_from_something_that_is_not_a_service_is_caught() {
        fault(
            "by = \"bazarr\"\nasks = \"library.curate\"",
            "by = \"subtitler\"\nasks = \"library.curate\"",
            "by names subtitler, which is not a service",
        );
    }

    #[test]
    fn a_wiring_to_something_that_is_not_a_service_is_caught() {
        fault(
            "by = \"unpackerr\"\nto = \"lidarr\"",
            "by = \"unpackerr\"\nto = \"lidaarr\"",
            "to names lidaarr, which is not a service",
        );
    }

    /// A plugin's own name has a colon and is that plugin's to declare. A link asking
    /// for one is asking for something no vocabulary could carry.
    #[test]
    fn a_wiring_asking_for_a_name_of_the_wrong_shape_is_caught() {
        fault(
            "by = \"seerr\"\nasks = \"identity.source\"",
            "by = \"seerr\"\nasks = \"plex:identity\"",
            "asks for plex:identity, which is not a core capability name",
        );
    }

    /// Choosing a claimant that does not claim it is choosing nothing, and the link
    /// would resolve to a service that cannot do what was asked.
    #[test]
    fn a_chosen_filler_that_does_not_provide_it_is_caught() {
        fault(
            "asks = \"indexer.search\"\nfilled_by = \"prowlarr\"",
            "asks = \"indexer.search\"\nfilled_by = \"sabnzbd\"",
            "chooses sabnzbd to fill indexer.search, which it does not provide",
        );
    }

    /// A choice with no reason beside it is indistinguishable from a rule somebody
    /// encoded, which is the thing being chosen instead of.
    #[test]
    fn a_chosen_filler_with_no_reason_is_caught() {
        fault(
            "filled_by = \"prowlarr\"\nwhy = ",
            "filled_by = \"prowlarr\"\n# why = ",
            "chooses prowlarr and does not say why",
        );
    }

    #[test]
    fn a_wiring_that_reaches_all_of_them_and_chooses_one_is_caught() {
        fault(
            "by = \"prowlarr\"\nasks = \"library.curate\"\neach = true",
            "by = \"prowlarr\"\nasks = \"library.curate\"\neach = true\nfilled_by = \"sonarr\"",
            "it does both only by meaning neither",
        );
    }

    #[test]
    fn a_by_name_wiring_carrying_what_only_an_ask_may_say_is_caught() {
        fault(
            "by = \"unpackerr\"\nto = \"sonarr\"",
            "by = \"unpackerr\"\nto = \"sonarr\"\neach = true",
            "carries each, which says something only an ask can say",
        );
    }

    #[test]
    fn a_wiring_from_a_service_to_itself_is_caught() {
        fault(
            "by = \"unpackerr\"\nto = \"radarr\"",
            "by = \"unpackerr\"\nto = \"unpackerr\"",
            "names the service it runs from; nothing wires to itself",
        );
    }

    #[test]
    fn the_same_ask_written_twice_is_caught() {
        fault(
            "by = \"lidarr\"\nasks = \"download.usenet\"",
            "by = \"lidarr\"\nasks = \"download.usenet\"\n\n[[wiring]]\nby = \"lidarr\"\n\
             asks = \"download.usenet\"",
            "names download.usenet more than once",
        );
    }

    /// The stack this binary ships declares links and every one of them holds.
    #[test]
    fn every_link_the_shipped_stack_declares_is_sound() {
        assert_eq!(messages(STACK), Vec::<String>::new());
    }

    /// A stack written before this table existed is still a stack, and one that
    /// declares no link declares none rather than being wrong about all of them.
    #[test]
    fn a_stack_that_declares_no_wiring_at_all_is_faulted_for_none_of_it() {
        let bare = "schema_version = 1\nstack_version = \"0.1.0\"\nmin_cli_version = \"0.1.0\"\n";
        assert_eq!(messages(bare), Vec::<String>::new());
    }
}
