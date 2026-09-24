//! Whether what a manifest claims and contributes holds against what this build
//! publishes.
//!
//! Two vocabularies decide it, and both are lemonfiber's: the capabilities a service
//! may claim, and the points a plugin may put a row at. A manifest is held to them
//! here rather than at the moment it is installed, because every one of these
//! refusals is about the *declaration* — and an author who has to install a plugin to
//! find out their claim is unbound is an author debugging somebody else's machine.
//!
//! Every violation is reported in one pass, each naming where it is. The refusals are
//! deliberately wordy on one side: a name that was not recognised is named alongside
//! the ones that were, because "no such capability" without the list is a riddle.
//!
//! What is **not** here is the rest of the contract — the required fields, the digest,
//! the forms, the paths. Those are the reader's, and this module is only the half that
//! needs a published set to decide at all.

mod contributing;

use std::collections::{BTreeMap, BTreeSet};

use crate::schema::{Claim, Expect, Manifest, Service};
use crate::vocabulary::{self, Capability, Constraint, Probe, Removed};
use crate::Violation;

/// Everything a manifest declares that the two published vocabularies refuse.
///
/// `occupied` is the identities the registers already hold, passed in for the reason
/// the extension points take it: a bundled check that is renamed has to move what a
/// contribution may collide with, rather than leaving a stale name free to be taken.
///
/// An empty answer is not a valid manifest. It means the two things this decides —
/// what the services claim, and what the plugin contributes — hold.
#[must_use]
pub fn violations(manifest: &Manifest, occupied: &[&str]) -> Vec<Violation> {
    let mut found = Vec::new();
    for service in &manifest.services {
        claimed_by(service, &manifest.plugin.id, &mut found);
    }
    demonstrated(manifest, &mut found);
    contributing::contributed(manifest, occupied, &mut found);
    found
}

/// The capabilities one service says it can do.
///
/// Two shapes and no third. A core name is lemonfiber's, comes from the published
/// vocabulary and has to be demonstrated; a namespaced one is this plugin's, is inert
/// until something asks for it, and has no published contract to satisfy.
fn claimed_by(service: &Service, plugin: &str, found: &mut Vec<Violation>) {
    let at = format!("service {}", service.id);
    for name in &service.provides {
        if vocabulary::is_core_name(name) {
            core(name, &at, vocabulary::removed(), found);
        } else if !namespaced_with(name, plugin) {
            found.push(Violation {
                location: format!("{at}.provides"),
                message: format!(
                    "{name} is neither a core name — an area, a dot and a verb — nor namespaced \
                     with this plugin's id ({plugin}:…), and a capability that is neither is one \
                     nothing can ask for"
                ),
            });
        }
    }
}

/// One core-shaped name, against the generation this build carries.
///
/// A name a published generation carried and this one does not is told which
/// generation it went in. That is a different fact from a name that never existed and
/// the only one of the two an author can act on, so the two refusals are not merged.
fn core(name: &str, at: &str, removed: &[Removed], found: &mut Vec<Violation>) {
    if vocabulary::carried().iter().any(|held| held.name == name) {
        return;
    }
    let location = format!("{at}.provides");
    if let Some(gone) = removed.iter().find(|one| one.name == name) {
        found.push(Violation {
            location,
            message: format!(
                "{name} was removed in capability vocabulary generation {}, so this claims a name \
                 this build no longer carries",
                gone.removed_in
            ),
        });
        return;
    }
    found.push(Violation {
        location,
        message: format!(
            "{name} names no capability the published vocabulary carries; it carries: {}",
            listed(vocabulary::carried().iter().map(|held| held.name))
        ),
    });
}

/// A declaration and its evidence, held to each other.
///
/// `provides` says what a service can do and `[[claim]]` shows it, so neither half
/// stands alone: a core name nothing demonstrates asserts, and a claim for a name no
/// service declares demonstrates something nobody said.
fn demonstrated(manifest: &Manifest, found: &mut Vec<Violation>) {
    let mut declared: BTreeMap<&str, &str> = BTreeMap::new();
    for service in &manifest.services {
        for name in service
            .provides
            .iter()
            .map(String::as_str)
            .filter(|name| vocabulary::is_core_name(name))
        {
            if let Some(first) = declared.insert(name, service.id.as_str()) {
                found.push(Violation {
                    location: format!("service {}.provides", service.id),
                    message: format!(
                        "{name} is a core capability and {first} already declares it; something \
                         asks for one of these by name and exactly one service answers, so two \
                         services of one plugin is not a choice an operator could make — it is \
                         two answers to one question, decided here rather than contested on their \
                         machine"
                    ),
                });
            }
        }
    }

    let mut claimed: BTreeSet<&str> = BTreeSet::new();
    for claim in &manifest.claims {
        let at = format!("claim {}", claim.capability);
        if !vocabulary::is_core_name(&claim.capability) {
            found.push(Violation {
                location: at,
                message: format!(
                    "{} is this plugin's own capability, and a namespaced capability has no \
                     published contract to satisfy — it is declared in `provides` and shown by a \
                     [[proof]] instead",
                    claim.capability
                ),
            });
            continue;
        }
        if !claimed.insert(&claim.capability) {
            found.push(Violation {
                location: at.clone(),
                message: format!("{} is claimed twice", claim.capability),
            });
        }
        if !declared.contains_key(claim.capability.as_str()) {
            found.push(Violation {
                location: at.clone(),
                message: format!(
                    "{} is in no service's `provides`, so nothing here has said it can do it",
                    claim.capability
                ),
            });
        }
        if let Some(held) = vocabulary::carried()
            .iter()
            .find(|held| held.name == claim.capability)
        {
            bound(claim, held, &at, found);
        }
    }

    for (name, service) in declared.iter().filter(|(name, _)| !claimed.contains(*name)) {
        found.push(Violation {
            location: format!("service {service}.provides"),
            message: format!(
                "{name} is declared and demonstrated by no [[claim]]; a capability is \
                 demonstrated, not asserted"
            ),
        });
    }
}

/// The probes a claim binds, against the ones its capability declares.
///
/// Every one of them, exactly once, and no others. The vocabulary owns what must be
/// shown and the claimant owns where to ask it, so a binding that is missing is a
/// contract half-satisfied and one that is invented is evidence for nothing.
fn bound(claim: &Claim, capability: &Capability, at: &str, found: &mut Vec<Violation>) {
    let declares = listed(capability.probes.iter().map(|probe| probe.id));
    let mut seen: BTreeSet<&str> = BTreeSet::new();

    for binding in &claim.probes {
        let location = format!("{at}.probe {}", binding.id);
        if !seen.insert(&binding.id) {
            found.push(Violation {
                location: location.clone(),
                message: format!("{} is bound twice", binding.id),
            });
        }
        let Some(probe) = capability
            .probes
            .iter()
            .find(|probe| probe.id == binding.id)
        else {
            found.push(Violation {
                location,
                message: format!(
                    "{} is not a probe {} declares; it declares: {declares}",
                    binding.id, capability.name
                ),
            });
            continue;
        };
        within(&binding.expect, probe, &location, found);
        asks_nothing_of_the_library(&binding.expect, &location, found);
    }

    for probe in capability.probes {
        if !seen.contains(probe.id) {
            found.push(Violation {
                location: at.to_owned(),
                message: format!(
                    "binds no probe {}, which {} declares; it declares: {declares}",
                    probe.id, capability.name
                ),
            });
        }
    }
}

/// One binding's expectation, against what its probe permits.
///
/// A claim demonstrated by the wrong evidence is an undemonstrated claim, and the two
/// ways that happens are a status the probe does not accept as an answer and a body
/// nothing is said about.
fn within(expect: &Expect, probe: &Probe, at: &str, found: &mut Vec<Violation>) {
    let permitted = listed(probe.requires.status.iter().map(u16::to_string));
    if !expect
        .status
        .is_some_and(|status| probe.requires.status.contains(&status))
    {
        found.push(Violation {
            location: at.to_owned(),
            message: match expect.status {
                Some(status) => format!(
                    "expects status {status}, and {} answers this probe with: {permitted}",
                    probe.id
                ),
                None => format!(
                    "expects no status, and {} is answered by one of: {permitted}",
                    probe.id
                ),
            },
        });
    }

    if probe.requires.body.is_empty() {
        return;
    }
    if !probe
        .requires
        .body
        .iter()
        .any(|constraint| carries(expect, *constraint))
    {
        found.push(Violation {
            location: at.to_owned(),
            message: format!(
                "constrains no body, and {} is shown by one of: {}",
                probe.id,
                listed(probe.requires.body.iter().map(|one| one.as_str()))
            ),
        });
    }
}

/// Whether an expectation says the thing a constraint is.
///
/// A key that is present and asserts nothing does not count. `json_is_absent = false`
/// is the case the rule was written for: it reads as a constraint and no runner
/// evaluates it, so treating it as evidence would let a claim satisfy a body
/// requirement by writing a word.
///
/// The same is true of every empty collection, and the rule was applied to one case out
/// of seven. `json_has_keys = []`, `json = {}`, `json_types = {}` and
/// `json_at_least = {}` all deserialise to `Some(empty)`, so each satisfied the body
/// requirement — and then the runner iterated an empty collection, found nothing wrong,
/// and reported the capability demonstrated on the evidence that something answered
/// `200`. A port proxy answers `200`.
///
/// `content_type = ""` and `body_starts_with = ""` are the string form of it: `contains`
/// and `starts_with` are both true of every body.
///
/// `json_array_min = 0` stays a constraint, and the difference is real: the shape check
/// behind it still requires the body to parse as an array, which is what the vocabulary
/// says *reads as a list* means.
fn carries(expect: &Expect, constraint: Constraint) -> bool {
    match constraint {
        Constraint::Status => expect.status.is_some(),
        Constraint::Json => expect.json.as_ref().is_some_and(|held| !held.is_empty()),
        Constraint::JsonHasKeys => expect
            .json_has_keys
            .as_ref()
            .is_some_and(|keys| !keys.is_empty()),
        Constraint::JsonTypes => expect
            .json_types
            .as_ref()
            .is_some_and(|held| !held.is_empty()),
        Constraint::JsonAtLeast => expect
            .json_at_least
            .as_ref()
            .is_some_and(|held| !held.is_empty()),
        Constraint::JsonArrayMin => expect.json_array_min.is_some(),
        Constraint::JsonIsAbsent => expect.json_is_absent == Some(true),
        Constraint::ContentType => expect
            .content_type
            .as_ref()
            .is_some_and(|kind| !kind.is_empty()),
        Constraint::BodyStartsWith => expect
            .body_starts_with
            .as_ref()
            .is_some_and(|front| !front.is_empty()),
    }
}

/// A probe gates an install, so it asks what the service does rather than what the
/// operator has put in it.
///
/// A count above zero is the only way an expectation can say something about how much
/// the operator holds; everything else it can say is about shape. So that is where the
/// rule bites, and it is the mistake both published plugins made within an hour of the
/// vocabulary existing — *at least one series* reads as the stronger proof and is a
/// plugin nobody can install until they have copied their library over.
///
/// A contributed check is deliberately not held to this. It reports on a running stack
/// rather than gating an install, so one that fails on a fresh machine is a check doing
/// its job.
fn asks_nothing_of_the_library(expect: &Expect, at: &str, found: &mut Vec<Violation>) {
    if let Some(least) = expect.json_array_min.filter(|least| *least > 0) {
        found.push(Violation {
            location: format!("{at}.expect.json_array_min"),
            message: format!(
                "at least {least} asserts the operator has put something there, and a probe gates \
                 an install; 0 says the answer reads as a list without saying how long it is"
            ),
        });
    }
    for (key, least) in expect.json_at_least.iter().flatten() {
        if *least > 0 {
            found.push(Violation {
                location: format!("{at}.expect.json_at_least"),
                message: format!(
                    "{key} at least {least} asserts the operator has put something there, and a \
                     probe gates an install; a contributed check may say this and a probe may not"
                ),
            });
        }
    }
}

/// Whether a name is this plugin's own: its id, a colon, and something after it.
pub(super) fn namespaced_with(name: &str, plugin: &str) -> bool {
    name.split_once(':')
        .is_some_and(|(prefix, rest)| prefix == plugin && !rest.is_empty())
}

/// A set of names as a refusal lists them: alphabetical, comma-separated.
///
/// Sorted rather than in declaration order, because a list a person is expected to
/// look their own mistake up in is a list they read rather than a list that was
/// convenient to produce.
pub(super) fn listed<T: AsRef<str>>(names: impl Iterator<Item = T>) -> String {
    let sorted: BTreeSet<String> = names.map(|name| name.as_ref().to_owned()).collect();
    sorted.into_iter().collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests;
