//! What a bundled service claims, read by the reader a plugin's claims are read by.
//!
//! The stack writes `[[service.claim]]` in the shape a plugin writes `[[claim]]`, and
//! the stack's own reader keeps those tables as it found them, because it cannot reach
//! this crate. They are read here, into the plugin's [`Claim`], and bound by the same
//! rules: every probe the capability declares, exactly once, and every expectation one
//! the probe permits. A bundled service and a plugin standing in for it therefore meet
//! one contract rather than two that could drift.
//!
//! What differs is where the evidence lives. A plugin carries its recordings in its own
//! source; the stack carries each service's under `recordings/<service id>/`, so a
//! recording named anywhere else is refused before anything is judged against it.

use std::collections::BTreeSet;

use serde::Deserialize as _;

use super::bound;
use crate::schema::Claim;
use crate::vocabulary;
use crate::Violation;

/// A bundled service's claims, and what refuses them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Bundled {
    /// The claims that could be read, whether or not anything refuses them.
    pub claims: Vec<Claim>,
    /// The capabilities in `provides` that no claim demonstrates.
    pub unclaimed: Vec<String>,
    /// Everything wrong with the claims that were written.
    pub violations: Vec<Violation>,
}

/// Read one bundled service's claims and hold them to the contract.
///
/// A capability in `provides` with no claim is returned apart from the violations,
/// because what it costs is the caller's to say: judged against recordings it is a
/// capability nothing has shown, which is unproven rather than refuted.
#[must_use]
pub fn bundled(service: &lemonfiber_manifest::Service) -> Bundled {
    let at = format!("service {}", service.id);
    let mut found = Vec::new();
    let mut claims = Vec::new();
    let mut claimed: BTreeSet<String> = BTreeSet::new();

    for (index, written) in service.claim.iter().enumerate() {
        let claim = match Claim::deserialize(toml::Value::Table(written.0.clone())) {
            Ok(claim) => claim,
            Err(why) => {
                found.push(Violation {
                    location: format!("{at}.claim[{index}]"),
                    message: format!("is not a claim this build can read: {why}"),
                });
                continue;
            }
        };
        let place = format!("{at}.claim {}", claim.capability);
        if !service.provides.contains(&claim.capability) {
            found.push(Violation {
                location: place.clone(),
                message: format!(
                    "{} is not in this service's `provides`, so it demonstrates something the \
                     service has not said it can do",
                    claim.capability
                ),
            });
        }
        if !claimed.insert(claim.capability.clone()) {
            found.push(Violation {
                location: place.clone(),
                message: format!("{} is claimed twice", claim.capability),
            });
        }
        kept_to(&service.id, &claim, &place, &mut found);
        match vocabulary::carried()
            .iter()
            .find(|held| held.name == claim.capability)
        {
            Some(held) => bound(&claim, held, &place, &mut found),
            None => found.push(Violation {
                location: place,
                message: format!(
                    "{} names no capability the published vocabulary carries",
                    claim.capability
                ),
            }),
        }
        claims.push(claim);
    }

    Bundled {
        unclaimed: service
            .provides
            .iter()
            .filter(|name| !claimed.contains(*name))
            .cloned()
            .collect(),
        claims,
        violations: found,
    }
}

/// Every recording a claim names, held to its own service's directory.
///
/// A recording under another service's directory is evidence about that service, and
/// one outside `recordings/` is not one the stack carries as evidence at all.
fn kept_to(service: &str, claim: &Claim, at: &str, found: &mut Vec<Violation>) {
    let under = format!("recordings/{service}/");
    for probe in &claim.probes {
        let beneath = probe
            .fixture
            .strip_prefix(&under)
            .filter(|rest| !rest.is_empty() && !rest.split('/').any(|step| step == ".."));
        if beneath.is_none() {
            found.push(Violation {
                location: format!("{at}.probe {}", probe.id),
                message: format!(
                    "{} is not under {under}, which is where this service's recordings are kept",
                    probe.fixture
                ),
            });
        }
    }
}

#[cfg(test)]
mod tests;
