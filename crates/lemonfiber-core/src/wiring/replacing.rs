//! The parts a choice of filler takes out of their form.

use std::collections::BTreeSet;

use lemonfiber_manifest::Manifest;

use super::{filled, settle, Chosen, Reaches, Settled, Whose};
use crate::plugin::Installed;

/// Every service the operator's choices in `chosen` take out of its form: each one a
/// choice passed over that fills nothing anything asks for, with the upstream each such
/// adapter fronts, in the order the stack and its plugins declare them.
#[must_use]
pub fn replaced(manifest: &Manifest, installed: &[Installed], chosen: &Chosen) -> Vec<String> {
    let wired = settle(manifest, installed, chosen);
    let filling: BTreeSet<String> = filled(&wired).into_values().flatten().collect();
    let passed_over: BTreeSet<&String> = wired
        .iter()
        .filter_map(|one| match &one.reaches {
            Reaches::Asked {
                settled:
                    Settled::Chosen {
                        whose: Whose::Operator,
                        over,
                        ..
                    },
                ..
            } => Some(over),
            _ => None,
        })
        .flatten()
        .filter(|service| !filling.contains(*service))
        .collect();
    let fronted: BTreeSet<&String> = installed
        .iter()
        .flat_map(|one| {
            one.services
                .iter()
                .filter(|placed| passed_over.contains(&placed.service))
                .filter_map(|placed| one.upstream_of(placed))
                .map(|upstream| &upstream.service)
        })
        .filter(|upstream| !filling.contains(*upstream))
        .collect();
    manifest
        .services
        .iter()
        .map(|service| &service.id)
        .chain(
            installed
                .iter()
                .flat_map(|one| one.services.iter().map(|placed| &placed.service)),
        )
        .filter(|id| passed_over.contains(id) || fronted.contains(id))
        .cloned()
        .collect()
}
