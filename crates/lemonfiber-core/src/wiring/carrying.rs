//! What a substitution leaves behind: each kind of thing the part it stops holds that the
//! replacement is not carried, read from what each contract declares its filler holds
//! and from no list kept per part.

use lemonfiber_manifest::Manifest;
use serde::Serialize;

use crate::plugin::Installed;

/// One kind of thing the part a substitution stops holds, and the replacement does not
/// receive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct LeftBehind {
    /// The capability whose contract declares it held.
    pub capability: String,
    /// The kind, by its stable id.
    pub kind: String,
    /// The words the rehearsal names it in.
    pub said: String,
}

impl std::fmt::Display for LeftBehind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.said, self.capability)
    }
}

/// Every kind a capability the part holding `was` fills declares held, less each kind the
/// contract of `capability` carries where that part and the one holding `now` both speak
/// it.
#[must_use]
pub fn left_behind(
    manifest: &Manifest,
    installed: &[Installed],
    capability: &str,
    was: &str,
    now: &str,
) -> Vec<LeftBehind> {
    let carried = |kind: &str| {
        contract(capability).is_some_and(|substituted| {
            speaks(installed, was, &substituted)
                && speaks(installed, now, &substituted)
                && substituted
                    .holds
                    .iter()
                    .any(|held| held.carried && held.kind == kind)
        })
    };
    filling(manifest, installed, was)
        .iter()
        .filter_map(|filled| contract(filled))
        .flat_map(|filled| {
            filled.holds.iter().map(move |held| LeftBehind {
                capability: filled.name.to_owned(),
                kind: held.kind.to_owned(),
                said: held.said.to_owned(),
            })
        })
        .filter(|held| held.capability != capability || !carried(&held.kind))
        .collect()
}

/// The contract this build speaks for `capability`, where it names one.
fn contract(capability: &str) -> Option<lemonfiber_contract::Capability> {
    lemonfiber_contract::capabilities::all()
        .into_iter()
        .find(|one| one.name == capability)
}

/// The services of the part `service` belongs to: every service of the plugin that
/// placed it, or the stack service alone.
fn part<'a>(installed: &'a [Installed], service: &str) -> Option<&'a Installed> {
    installed
        .iter()
        .find(|one| one.services.iter().any(|placed| placed.service == service))
}

/// Every capability the part holding `service` fills, in the order its services declare
/// them.
fn filling(manifest: &Manifest, installed: &[Installed], service: &str) -> Vec<String> {
    let provided: Vec<String> = match part(installed, service) {
        Some(plugin) => plugin
            .services
            .iter()
            .flat_map(|placed| placed.provides.iter().cloned())
            .collect(),
        None => manifest
            .services
            .iter()
            .filter(|one| one.id == service)
            .flat_map(|one| one.provides.iter().cloned())
            .collect(),
    };
    let mut seen = std::collections::BTreeSet::new();
    provided
        .into_iter()
        .filter(|one| seen.insert(one.clone()))
        .collect()
}

/// Whether a service of the part holding `service` speaks `contract`; a stack service
/// speaks none.
fn speaks(
    installed: &[Installed],
    service: &str,
    contract: &lemonfiber_contract::Capability,
) -> bool {
    let spoken = lemonfiber_contract::spoken(contract.name, contract.major);
    part(installed, service).is_some_and(|plugin| {
        plugin
            .services
            .iter()
            .any(|placed| placed.speaks.contains(&spoken))
    })
}
