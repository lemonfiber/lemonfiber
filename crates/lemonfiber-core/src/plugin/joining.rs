//! The networks a plugin's service joins beside the stack's own.
//!
//! The stack keeps some reach on networks of its own: the request service reaches the
//! curators it hands requests to and the media server it signs in against only through
//! the request gate, over a network the gate shares with those services alone. A
//! plugin's service standing in for one of them is reached that way only if it is on
//! the same networks, so it joins them — exactly the networks of the stack services it
//! stands in for, and no other.
//!
//! **What it stands in for is read from what it declared, never from a name.** A stack
//! service it stands in for speaks the adapter it names, provides a capability it
//! provides, and — where the stack service files media — files a medium the plugin's
//! service files too. A film curator stands in for the stack's film curator and not for
//! its music one; a service naming no adapter, or no capability the stack's services
//! provide, stands in for nothing and stays on the default network alone. The networks
//! themselves are the stack's: a plugin has no field in which to name one.
//!
//! **A network kept for a link by name is not joined.** Where every other service on a
//! network reaches the stack service by name, the network carries a link that is about
//! that one service and never about whatever stands in for it, so a stand-in is never
//! reached over it and is given no route to what is on it.
//!
//! **Settled rather than declared.** A plugin's service stands in for a stack service
//! only through one of the stack's own asks it is settled to fill: the one the operator
//! chose it for, or one every claimant answers. A plugin claiming what the stack's own
//! service still answers replaces nothing, and joining that service's networks would
//! give it reach the stack never granted it. Choosing a filler is what changes this, so
//! a choice rewrites the documents of the plugins whose networks it moves.

use std::collections::{BTreeMap, BTreeSet};

use lemonfiber_manifest::{ApiKind, Manifest};

use super::installed::Placed;
use crate::stack::attached::DEFAULT;

/// The stack's services as what a plugin's service could stand in for, each with the
/// networks it is on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Joins {
    /// Every stack service that speaks an adapter, in the manifest's order.
    standing: Vec<Standing>,
    /// Each plugin's service the stack's asks are settled to reach, by the plugin and the
    /// service, with what they ask it for.
    fills: BTreeMap<(String, String), BTreeSet<String>>,
}

/// One stack service, as what a plugin's service could stand in for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Standing {
    /// The adapter it speaks.
    speaks: ApiKind,
    /// What it provides.
    provides: Vec<String>,
    /// The media it files.
    files: Vec<String>,
    /// The networks it is on.
    on: BTreeSet<String>,
}

impl Joins {
    /// The stack's services and the networks its compose files put each on, beside what
    /// each of the stack's asks is settled to reach.
    #[must_use]
    pub fn of(
        manifest: &Manifest,
        attached: &BTreeMap<String, BTreeSet<String>>,
        settled: &[crate::wiring::Wired],
    ) -> Self {
        // A settled answer names a service by its id alone, so one a stack service also
        // carries cannot be told apart from the stack's own: it is taken as the stack's,
        // and no plugin's service is settled by a name it shares.
        let mut fills: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
        for wired in settled
            .iter()
            .filter(|wired| wired.origin == crate::origin::Origin::Bundled)
        {
            let crate::wiring::Reaches::Asked {
                capability,
                services,
                origins,
                ..
            } = &wired.reaches
            else {
                continue;
            };
            for service in services {
                let Some(crate::origin::Origin::Plugin { named }) = origins.get(service) else {
                    continue;
                };
                if manifest.services.iter().any(|one| one.id == *service) {
                    continue;
                }
                fills
                    .entry((named.clone(), service.clone()))
                    .or_default()
                    .insert(capability.clone());
            }
        }
        let standing = manifest
            .services
            .iter()
            .filter_map(|service| {
                Some(Standing {
                    speaks: service.api.as_ref()?.kind,
                    provides: service.provides.clone(),
                    files: service.media_types.clone(),
                    on: attached
                        .get(&service.id)?
                        .iter()
                        .filter(|network| !by_name_only(manifest, attached, &service.id, network))
                        .cloned()
                        .collect(),
                })
            })
            .collect();
        Self { standing, fills }
    }

    /// The networks `placed`, one of `plugin`'s services, joins: every network a stack
    /// service it stands in for is on, through a capability it declares and is settled to
    /// fill, or nothing where that is the default network alone, which it is on already.
    #[must_use]
    pub fn of_service(&self, plugin: &str, placed: &Placed) -> Vec<String> {
        let Some(speaks) = placed.api.as_ref().map(|api| api.kind) else {
            return Vec::new();
        };
        let Some(settled) = self.fills.get(&(plugin.to_owned(), placed.service.clone())) else {
            return Vec::new();
        };
        let fills: BTreeSet<&String> = settled
            .iter()
            .filter(|it| placed.provides.contains(it))
            .collect();
        let joined: BTreeSet<&String> = self
            .standing
            .iter()
            .filter(|one| one.speaks == speaks)
            .filter(|one| one.provides.iter().any(|it| fills.contains(&it)))
            .filter(|one| {
                one.files.is_empty() || one.files.iter().any(|it| placed.media_types.contains(it))
            })
            .flat_map(|one| &one.on)
            .collect();
        if joined.iter().all(|network| *network == DEFAULT) {
            return Vec::new();
        }
        joined.into_iter().cloned().collect()
    }
}

/// Whether every other service on `network` reaches `id` by name, which makes the
/// network one kept for links about that service alone.
fn by_name_only(
    manifest: &Manifest,
    attached: &BTreeMap<String, BTreeSet<String>>,
    id: &str,
    network: &str,
) -> bool {
    let mut others = attached
        .iter()
        .filter(|(other, on)| other.as_str() != id && on.contains(network))
        .map(|(other, _)| other)
        .peekable();
    others.peek().is_some()
        && others.all(|other| {
            manifest
                .wirings
                .iter()
                .any(|wiring| wiring.by == *other && wiring.to.as_deref() == Some(id))
        })
}

#[cfg(test)]
mod tests;
