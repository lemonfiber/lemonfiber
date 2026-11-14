//! The services of a first-party plugin, in the stack profile its bundle entry places them
//! in, whatever its stage.
//!
//! A plugin the bundle assembles or installs takes the place of one of the stack's parts,
//! and one it offers stands in for one, so its services start and stop with the forms
//! whose closure holds that profile, are narrowed away with that profile's protocol, and
//! are surveyed and waited on beside the stack's own services. Any other plugin's services
//! sit in a profile of the plugin's own, which joins the closure of each form its manifest
//! names.

use lemonfiber_bundled::{Pinned, PINNED};
use lemonfiber_manifest::Criticality;

use super::Installed;

/// One service of a plugin the bundle installs or offers, as a service of the stack
/// profile it sits in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profiled {
    /// Its id, which is its Compose service name.
    pub id: String,
    /// What it is called in front of an operator.
    pub name: String,
    /// What it does for the operator, in its plugin's words.
    pub describes: String,
    /// The stack profile it sits in.
    pub profile: String,
    /// How much its absence costs, as its plugin declares it.
    pub criticality: Criticality,
}

/// The bundle's pin of `installed` and the stack profile it names, where `installed` is a
/// first-party plugin the bundle places in one.
fn pinned(installed: &Installed) -> Option<(&'static Pinned, &'static str)> {
    pinned_in(PINNED, installed)
}

/// The pin of `installed` among `pins` and the stack profile it names, whatever its stage.
fn pinned_in<'p>(pins: &'p [Pinned], installed: &Installed) -> Option<(&'p Pinned, &'p str)> {
    pins.iter()
        .filter(|pin| super::first_party::holds(std::slice::from_ref(&pin.first_party), installed))
        .find_map(|pin| pin.profile.map(|profile| (pin, profile)))
}

/// The Compose profile `installed`'s services sit in: the stack profile its bundle entry
/// names where it is a first-party plugin, and the plugin's own otherwise.
#[must_use]
pub fn profile_of(installed: &Installed) -> String {
    pinned(installed).map_or_else(
        || super::profile(&installed.plugin),
        |(_, profile)| profile.to_owned(),
    )
}

/// Every service of `installed` that sits in a stack profile, in the order each plugin
/// declares them.
#[must_use]
pub fn profiled(installed: &[Installed]) -> Vec<Profiled> {
    let mut found = Vec::new();
    for one in installed {
        let Some((pin, profile)) = pinned(one) else {
            continue;
        };
        let declared = lemonfiber_plugin::Manifest::from_toml(pin.copy)
            .map(|manifest| manifest.services)
            .unwrap_or_default();
        for (service, placed) in declared.iter().filter_map(|service| {
            one.services
                .iter()
                .find(|placed| placed.service == service.id)
                .map(|placed| (service, placed))
        }) {
            found.push(Profiled {
                id: placed.service.clone(),
                name: placed.called().to_owned(),
                describes: placed.description.clone(),
                profile: profile.to_owned(),
                criticality: criticality(service.criticality),
            });
        }
    }
    found
}

/// Every plugin of `installed` whose services sit in a profile of its own, with the forms
/// that profile joins.
#[must_use]
pub fn joining(installed: &[Installed]) -> Vec<crate::stack::closure::Joining> {
    installed
        .iter()
        .filter(|one| pinned(one).is_none())
        .map(|one| crate::stack::closure::Joining {
            plugin: one.plugin.clone(),
            profile: super::profile(&one.plugin),
            services: one
                .services
                .iter()
                .map(|placed| placed.service.clone())
                .collect(),
            forms: one.forms.clone(),
        })
        .collect()
}

/// A plugin's criticality as the stack's vocabulary names the same weight.
const fn criticality(declared: lemonfiber_plugin::Criticality) -> Criticality {
    match declared {
        lemonfiber_plugin::Criticality::Critical => Criticality::Critical,
        lemonfiber_plugin::Criticality::Core => Criticality::Core,
        lemonfiber_plugin::Criticality::Important => Criticality::Important,
        lemonfiber_plugin::Criticality::Enhancing => Criticality::Enhancing,
        lemonfiber_plugin::Criticality::Optional => Criticality::Optional,
    }
}

#[cfg(test)]
mod tests;
