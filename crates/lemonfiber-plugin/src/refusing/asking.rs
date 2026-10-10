//! What a plugin's services may ask for: core capabilities, never a service by name.

use std::collections::BTreeSet;

use crate::schema::Manifest;
use crate::{claiming, vocabulary, Violation};

/// Every ask held to its own service and to the published core vocabulary.
pub(super) fn asked(manifest: &Manifest, found: &mut Vec<Violation>) {
    let one = manifest.services.len() == 1;
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    for (at, ask) in manifest.asking.iter().enumerate() {
        let location = ask
            .service
            .as_deref()
            .map_or_else(|| format!("ask #{}", at + 1), |id| format!("ask {id}"));
        let named = format!("{location}.capability");
        if vocabulary::is_core_name(&ask.capability) {
            claiming::core(&ask.capability, &named, vocabulary::removed(), found);
        } else {
            found.push(Violation {
                location: named,
                message: format!(
                    "{} is not a core capability; a plugin asks only by core capability, and a \
                     namespaced one carries the id of the plugin it belongs to, so asking for it \
                     would name that plugin",
                    ask.capability
                ),
            });
        }
        let Some(service) =
            super::naming::names(manifest, ask.service.as_deref(), &location, one, found)
        else {
            continue;
        };
        if !seen.insert((service.id.as_str(), ask.capability.as_str())) {
            found.push(Violation {
                location: location.clone(),
                message: format!("{} asks for {} twice", service.id, ask.capability),
            });
        }
        if service.provides.contains(&ask.capability) {
            found.push(Violation {
                location,
                message: format!(
                    "{} asks for {}, which it provides itself; nothing asks itself",
                    service.id, ask.capability
                ),
            });
        }
    }
}

#[cfg(test)]
mod tests;
