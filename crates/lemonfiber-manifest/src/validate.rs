//! Checking a manifest against the contract.
//!
//! Every violation is reported in one pass, each naming where it is. Stopping at
//! the first turns fixing a fork into a guessing game: change one line, run
//! again, find the next, repeat — when the whole list was knowable at once.
//!
//! Validation is separate from parsing because they fail for different reasons
//! and deserve different answers. A file that is not a manifest is a syntax
//! error; a manifest that contradicts itself parsed perfectly well.

use std::collections::{BTreeMap, BTreeSet};

use crate::{ApiKind, Date, Manifest, Protocol, Removed, Service};

mod wiring;

/// Tags that move under you. A pin meaning "whatever is newest" is not a pin.
const FLOATING_TAGS: &[&str] = &[
    "latest", "stable", "edge", "nightly", "develop", "dev", "main", "master", "rolling",
];

/// The kernel capabilities a service may be granted.
///
/// Deliberately one entry. A kernel capability is a hole in the isolation the
/// stack otherwise relies on, and the tunnel genuinely needs this one to build
/// an interface. Anything else should have to argue for itself in a spec change.
///
/// Public because it is one of two sets of things called capabilities in this system,
/// and the rule that no name may be in both is only checkable by something that can
/// see both.
pub const ALLOWED_GRANTS: &[&str] = &["NET_ADMIN"];

/// Whether a name has the shape of a core capability: an area, a dot and a verb.
///
/// The one definition of that shape. Which kind a name is has to be decidable by
/// reading it — that is what lets a plugin's own namespaced capability be told from
/// lemonfiber's in a listing neither of them wrote, and what makes a core-looking name
/// no vocabulary carries a name that is *missing* rather than somebody's namespace.
///
/// Here rather than beside the vocabulary because both readers of the shape are
/// downstream of this crate: a stack's `provides` is checked here, a plugin's against
/// the published set, and two spellings of one shape is a name one reader accepts and
/// the other refuses.
#[must_use]
pub fn is_core_name(name: &str) -> bool {
    let mut halves = name.split('.');
    match (halves.next(), halves.next(), halves.next()) {
        (Some(area), Some(verb), None) => word(area) && word(verb),
        _ => false,
    }
}

/// One half of a core name: lowercase, starting with a letter, hyphens inside it.
fn word(half: &str) -> bool {
    half.starts_with(|first: char| first.is_ascii_lowercase())
        && !half.ends_with('-')
        && half
            .chars()
            .all(|each| each.is_ascii_lowercase() || each.is_ascii_digit() || each == '-')
}

/// The OSI-approved identifiers a service licence may use.
const OSI: &str = include_str!("spdx_osi.txt");

/// The identifiers a vendored list holds, ignoring the prose it explains itself with.
fn identifiers(list: &str) -> BTreeSet<&str> {
    list.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// One thing wrong with a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Where it is, in the manifest's own terms — `service sonarr`, `form tv`.
    pub location: String,
    /// What is wrong, in one line.
    pub message: String,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.location, self.message)
    }
}

/// Check a manifest against the contract, reporting everything wrong with it.
///
/// `today` is supplied rather than read so the result depends only on its
/// inputs — a validator that consults the clock passes and fails on different
/// days for the same file.
#[must_use]
pub fn validate(manifest: &Manifest, today: Date) -> Vec<Violation> {
    let mut found = Vec::new();
    let profiles = check_profiles(manifest, &mut found);
    check_forms(manifest, &profiles, &mut found);
    check_services(manifest, &profiles, today, &mut found);
    check_removed(manifest, &mut found);
    wiring::check(manifest, &mut found);
    found
}

/// Profile ids must be unique, and each protocol claimed at most once.
fn check_profiles(manifest: &Manifest, found: &mut Vec<Violation>) -> BTreeSet<String> {
    let mut declared = BTreeSet::new();
    let mut claimed: BTreeMap<Protocol, String> = BTreeMap::new();

    for profile in &manifest.profiles {
        let location = format!("profile {}", profile.id);
        if !declared.insert(profile.id.clone()) {
            found.push(Violation {
                location: location.clone(),
                message: "another profile already has this id".to_owned(),
            });
        }
        if let Some(protocol) = profile.protocol {
            if let Some(owner) = claimed.get(&protocol) {
                found.push(Violation {
                    location,
                    message: format!("profile {owner} already carries this protocol"),
                });
            } else {
                claimed.insert(protocol, profile.id.clone());
            }
        }
    }
    declared
}

/// Form ids must be unique, and every profile a form names must exist.
fn check_forms(manifest: &Manifest, profiles: &BTreeSet<String>, found: &mut Vec<Violation>) {
    let mut declared = BTreeSet::new();
    for form in &manifest.forms {
        let location = format!("form {}", form.id);
        if !declared.insert(form.id.clone()) {
            found.push(Violation {
                location: location.clone(),
                message: "another form already has this id".to_owned(),
            });
        }
        for named in &form.profiles {
            if !profiles.contains(named) {
                found.push(Violation {
                    location: location.clone(),
                    message: format!("names profile {named}, which is not declared"),
                });
            }
        }
    }
}

/// Everything a service has to get right.
///
/// One rule per function, and the order they are chained in is the order an
/// operator reads them. Each answers for itself and returns what it found, so
/// adding a rule is adding a link rather than editing a body that already holds
/// eight others.
fn check_services(
    manifest: &Manifest,
    profiles: &BTreeSet<String>,
    today: Date,
    found: &mut Vec<Violation>,
) {
    let osi = identifiers(OSI);

    let of_service: BTreeMap<&str, &str> = manifest
        .services
        .iter()
        .map(|service| (service.id.as_str(), service.profile.as_str()))
        .collect();

    let mut declared = BTreeSet::new();
    for service in &manifest.services {
        let repeated = (!declared.insert(service.id.clone()))
            .then(|| "another service already has this id".to_owned());

        let faults = repeated
            .into_iter()
            .chain(placed(service, profiles))
            .chain(pinned(service))
            .chain(published(service))
            .chain(estimated(service))
            .chain(licensed(service, &osi))
            .chain(released(service, today))
            .chain(permitted(service))
            .chain(versioned(service))
            .chain(outbound(service))
            .chain(offered(service))
            .chain(depended(service, &of_service));

        let location = format!("service {}", service.id);
        found.extend(faults.map(|message| Violation {
            location: location.clone(),
            message,
        }));
    }
}

/// Everything a record of a dropped service has to get right.
///
/// Chained the way a service's rules are, and for the same reason: a fork that
/// recorded a removal badly should be told everything about it at once rather than
/// once per run.
fn check_removed(manifest: &Manifest, found: &mut Vec<Violation>) {
    let declared: BTreeSet<&str> = manifest
        .services
        .iter()
        .map(|service| service.id.as_str())
        .collect();
    let recorded: BTreeSet<&str> = manifest
        .removed
        .iter()
        .map(|removed| removed.id.as_str())
        .collect();

    let mut seen = BTreeSet::new();
    for removed in &manifest.removed {
        let repeated = (!seen.insert(removed.id.clone()))
            .then(|| "another removal already has this id".to_owned());

        let faults = repeated
            .into_iter()
            .chain(gone(removed, &declared))
            .chain(dated(removed))
            .chain(explained(removed))
            .chain(replaced(removed, &declared, &recorded));

        let location = format!("removed {}", removed.id);
        found.extend(faults.map(|message| Violation {
            location: location.clone(),
            message,
        }));
    }
}

/// A service recorded as removed is not also declared.
///
/// The two records contradict each other outright, and the contradiction is silent
/// where it matters most: an operator asking what became of a service would be told
/// it went, while the stack goes on starting it.
fn gone(removed: &Removed, declared: &BTreeSet<&str>) -> Option<String> {
    declared
        .contains(removed.id.as_str())
        .then(|| "is recorded as removed and is still declared as a service".to_owned())
}

/// A removal says which stack version stopped carrying it.
///
/// Emptiness rather than shape. The manifest validates neither `stack_version` nor
/// `min_cli_version` as semantic versions, and a rule here that parsed one would be
/// stricter about the past than the contract is about the present — so what is checked
/// is that the record names something, since a record that cannot be placed in the
/// stack's own history answers *when did this go* with nothing.
fn dated(removed: &Removed) -> Option<String> {
    removed
        .removed_in
        .trim()
        .is_empty()
        .then(|| "records no stack version it went in".to_owned())
}

/// A removal says why, in something more than an empty string.
///
/// Emptiness rather than presence, because presence is what the parse already
/// guarantees and an empty reason is the shape a record takes when somebody filled
/// the table in to satisfy it. The requirement is that the reason is recorded; a
/// field holding nothing records nothing.
fn explained(removed: &Removed) -> Option<String> {
    removed
        .reason
        .trim()
        .is_empty()
        .then(|| "records no reason for going".to_owned())
}

/// A replacement, where one is named, is something this stack knows about.
///
/// Either a service it declares or another service it recorded as removed — the
/// second because replacements chain, and a stack that dropped the thing that
/// replaced the thing it dropped has told the truth twice. A name that is neither
/// points an operator at nothing, which is worse than recording no replacement at
/// all, since it reads as an answer.
fn replaced(
    removed: &Removed,
    declared: &BTreeSet<&str>,
    recorded: &BTreeSet<&str>,
) -> Option<String> {
    let named = removed.replaced_by.as_deref()?;
    if named.trim().is_empty() {
        return Some("names an empty replacement rather than none at all".to_owned());
    }
    if named == removed.id {
        return Some("is recorded as having replaced itself".to_owned());
    }
    (!declared.contains(named) && !recorded.contains(named)).then(|| {
        format!("says {named} replaced it, and this stack neither declares nor records that")
    })
}

/// A service that says where it reaches says what it asks for there, and the
/// other way round.
///
/// Half of that pair is not a smaller answer, it is a misleading one. An inventory
/// of what leaves a machine reads an empty destination as "this service reaches
/// nothing", so a stack that named a purpose and no destination would have that
/// purpose attributed to a service the same report says goes nowhere. Refused by
/// name here rather than papered over at the point of reading, so whoever wrote the
/// manifest is the one who decides which half was meant.
fn outbound(service: &Service) -> Option<String> {
    match (&service.reaches, &service.asks_for) {
        (Some(_), None) => Some("says where it reaches and not what it asks for".to_owned()),
        (None, Some(_)) => Some("says what it asks for and not where it reaches".to_owned()),
        (Some(_), Some(_)) | (None, None) => None,
    }
}

/// A Servarr-shape service names the version of the API its client speaks.
///
/// The shape spans two — Sonarr and Radarr at v3, Lidarr and Prowlarr at v1 — so
/// the version cannot be assumed. A servarr service that omits it is a fault
/// surfaced here, rather than one that silently drops the service from seeding
/// and the doctor because it cannot be reached at a known path.
fn versioned(service: &Service) -> Option<String> {
    let api = service.api.as_ref()?;
    (api.kind == ApiKind::Servarr && api.version.is_none())
        .then(|| "has the servarr API shape but names no api.version".to_owned())
}

/// A service belongs to a profile the stack declares.
fn placed(service: &Service, profiles: &BTreeSet<String>) -> Option<String> {
    (!profiles.contains(&service.profile))
        .then(|| format!("is in profile {}, which is not declared", service.profile))
}

/// A service names a version rather than a tag that moves under it.
fn pinned(service: &Service) -> Option<String> {
    if service.tag.is_empty() {
        return Some("declares an empty image tag".to_owned());
    }
    FLOATING_TAGS.contains(&service.tag.as_str()).then(|| {
        format!(
            "is pinned to {}, which moves — that is not a pin",
            service.tag
        )
    })
}

/// A service that publishes a port says which interface it publishes on.
fn published(service: &Service) -> Option<String> {
    (service.port.is_some() && service.bind.is_none())
        .then(|| "publishes a port and does not say which interface".to_owned())
}

/// A memory estimate, where one is declared, is some memory.
fn estimated(service: &Service) -> Option<String> {
    (service.memory_mib == Some(0))
        .then(|| "estimates it needs no memory at all, which is not an estimate".to_owned())
}

/// A service declares a licence anyone can look up.
fn licensed(service: &Service, osi: &BTreeSet<&str>) -> Option<String> {
    (!osi.contains(service.license.as_str())).then(|| {
        format!(
            "declares licence {}, which is not a recognised OSI identifier",
            service.license
        )
    })
}

/// A service records a release date that is a date, and has happened.
fn released(service: &Service, today: Date) -> Option<String> {
    match Date::parse(&service.last_release) {
        None => Some(format!(
            "records last_release {}, which is not YYYY-MM-DD",
            service.last_release
        )),
        Some(recorded) if recorded > today => Some(format!(
            "records last_release {}, which is in the future",
            service.last_release
        )),
        Some(_) => None,
    }
}

/// A service offers each capability by a core name, and offers none of them twice.
///
/// Shape and nothing else. Whether a name is one the published vocabulary carries is
/// that vocabulary's question and not this crate's — it is generated from this field,
/// so a reader holding the set would be the cycle, and one holding a copy would be a
/// second answer. What can be settled here is that a core name is `area.verb`: one dot,
/// lowercase, and no colon, which is the shape a plugin's own namespaced capability
/// takes and a bundled service may not.
fn offered(service: &Service) -> Vec<String> {
    let mut seen = BTreeSet::new();
    service
        .provides
        .iter()
        .flat_map(|named| {
            let shape = (!is_core_name(named))
                .then(|| format!("provides {named}, which is not a core capability name"));
            let repeated =
                (!seen.insert(named.as_str())).then(|| format!("provides {named} more than once"));
            shape.into_iter().chain(repeated)
        })
        .collect()
}

/// A service asks only for the kernel capabilities the stack is willing to grant.
fn permitted(service: &Service) -> Vec<String> {
    service
        .grants
        .iter()
        .filter(|granted| !ALLOWED_GRANTS.contains(&granted.as_str()))
        .map(|granted| format!("asks for kernel capability {granted}, which is not allowed"))
        .collect()
}

/// A service waits only on things that will be running when it is.
///
/// Dependencies are allowed to exist; crossing a profile is what is not. A
/// service waiting on something that may not be running is a start-up that
/// hangs for a reason nothing reports.
fn depended(service: &Service, of_service: &BTreeMap<&str, &str>) -> Vec<String> {
    service
        .depends_on
        .iter()
        .filter_map(|needed| match of_service.get(needed.as_str()) {
            None => Some(format!("depends on {needed}, which is not a service here")),
            Some(other) if *other != service.profile => Some(format!(
                "depends on {needed}, which is in profile {other} rather than {}",
                service.profile
            )),
            Some(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests;
