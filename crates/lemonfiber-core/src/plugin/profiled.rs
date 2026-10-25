//! The services of a plugin the bundle installs, in the stack profile its bundle entry
//! places them in.
//!
//! A plugin the bundle installs takes the place of one of the stack's parts, so its
//! services start and stop with the forms whose closure holds that part's profile, are
//! narrowed away with that profile's protocol, and are surveyed and waited on beside the
//! stack's own services. Any other plugin's services sit in a profile of the plugin's
//! own, which no form names.

use lemonfiber_bundled::{Pinned, Stage, PINNED};
use lemonfiber_manifest::Criticality;

use super::Installed;

/// One service of a plugin the bundle installs, as a service of the stack profile it sits
/// in.
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
/// first-party plugin the bundle installs.
fn pinned(installed: &Installed) -> Option<(&'static Pinned, &'static str)> {
    let pin = PINNED
        .iter()
        .filter(|pin| pin.stage == Some(Stage::Installed))
        .find(|pin| super::first_party::holds(std::slice::from_ref(&pin.first_party), installed))?;
    Some((pin, pin.profile?))
}

/// The Compose profile `installed`'s services sit in: the stack profile its bundle entry
/// names where the bundle installs it, and the plugin's own otherwise.
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
        let Ok(declared) = lemonfiber_plugin::Manifest::from_toml(pin.copy) else {
            continue;
        };
        for service in &declared.services {
            let Some(placed) = one
                .services
                .iter()
                .find(|placed| placed.service == service.id)
            else {
                continue;
            };
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
