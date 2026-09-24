//! What each link comes to, given everything on the machine that claims what it asks.
//!
//! **Everything that claims it means everything installed**, the stack's own services
//! and every installed plugin's alike. A plugin's service that claims what the stack
//! asks for is a candidate on the same terms as a bundled one, and where there is more
//! than one candidate the ask is contested and refused until somebody chooses — never
//! settled by install order, precedence or recency. Reading the stack alone would
//! settle that contest in the stack's favour by leaving the plugin out, which is
//! resolving it by precedence with the precedence hidden.
//!
//! **And each candidate says where it came from.** Every claimant is named beside its
//! origin in the answer, so an operator reading a contest, or a wiring a plugin fills,
//! reads which of the names is not the stack's in the same place as the names.

use std::collections::BTreeMap;

use lemonfiber_manifest::{Manifest, Wiring};

use crate::filling::{fills, Claimant, Filling, Shown};
use crate::origin::Origin;
use crate::plugin::Installed;

use super::{Chosen, Contest, Reaches, Settled, Unfilled, Whose, Wired};

/// Everything installed that claims a capability: the stack's services in declaration
/// order, then each installed plugin's in the order the record holds them.
///
/// `Claimed` for both. A declaration is what the manifest holds and running a probe is
/// a separate act; a plugin's declaration has already been held to its demonstrations
/// before it could be installed, and the stack's is read as it always was.
pub(super) fn claimants(
    manifest: &Manifest,
    installed: &[Installed],
    capability: &str,
) -> Vec<Claimant> {
    let bundled = manifest
        .services
        .iter()
        .filter(|service| service.provides.iter().any(|named| named == capability))
        .map(|service| Claimant {
            service: service.id.clone(),
            plugin: None,
            shown: Shown::Claimed,
        });
    let brought = installed.iter().flat_map(|one| {
        one.services
            .iter()
            .filter(|placed| placed.provides.iter().any(|named| named == capability))
            .map(|placed| Claimant {
                service: placed.service.clone(),
                plugin: Some(one.plugin.clone()),
                shown: Shown::Claimed,
            })
    });
    bundled.chain(brought).collect()
}

/// Where each claimant came from, keyed by the service it is.
fn origins(held: &[Claimant]) -> BTreeMap<String, Origin> {
    held.iter()
        .map(|one| {
            (
                one.service.clone(),
                one.plugin
                    .clone()
                    .map_or(Origin::Bundled, |named| Origin::Plugin { named }),
            )
        })
        .collect()
}

/// What one ask comes to, given its claimants and whatever has been chosen.
fn asked(
    wiring: &Wiring,
    capability: &str,
    held: &[Claimant],
    chosen: &Chosen,
) -> (Vec<String>, Settled) {
    let names: Vec<String> = held.iter().map(|one| one.service.clone()).collect();
    if wiring.each && !names.is_empty() {
        return (names, Settled::Each);
    }

    // The operator's choice over the stack's, because the stack's is the default they
    // were offered and theirs is the answer they gave. Either is only a choice while
    // the service it names still claims the capability — one that stopped claiming it
    // at a pin bump leaves a contest rather than a filler nothing can demonstrate.
    //
    // And only where there is something to choose between. One service claiming it is
    // answered by the one service, not by crediting whoever wrote the setting with a
    // decision they were never offered.
    let picked = chosen
        .filler(capability)
        .map(|service| (service, Whose::Operator, None))
        .or_else(|| {
            wiring
                .filled_by
                .as_deref()
                .map(|service| (service, Whose::Stack, wiring.why.clone()))
        })
        .filter(|(service, _, _)| names.len() > 1 && names.iter().any(|one| one == service));

    match picked {
        Some((service, whose, why)) => (
            vec![service.to_owned()],
            Settled::Chosen {
                whose,
                why,
                over: names
                    .iter()
                    .filter(|one| *one != service)
                    .cloned()
                    .collect(),
            },
        ),
        // Nobody chose, so the claimants answer for themselves — through the one
        // function that decides what a set of claims comes to, rather than through a
        // second opinion about it kept here.
        None => match fills(held) {
            Filling::By { service } => (vec![service], Settled::Outright),
            Filling::Contested { claimants } => (Vec::new(), Settled::Contested { claimants }),
            Filling::Unfilled => (Vec::new(), Settled::Unfilled),
        },
    }
}

/// Every link the stack declares, answered against everything installed that claims
/// what it asks.
///
/// A link naming a `why` it should not have, or asking for something of the wrong
/// shape, is the validator's business rather than this one's: what is read here is a
/// manifest that has already been checked, and a reader that second-guessed it would
/// be a second opinion on the same file.
#[must_use]
pub fn settle(manifest: &Manifest, installed: &[Installed], chosen: &Chosen) -> Vec<Wired> {
    manifest
        .wirings
        .iter()
        .filter_map(|wiring| {
            let reaches = match (wiring.asks.as_deref(), wiring.to.as_deref()) {
                (Some(capability), None) => {
                    let held = claimants(manifest, installed, capability);
                    let (services, settled) = asked(wiring, capability, &held, chosen);
                    Reaches::Asked {
                        capability: capability.to_owned(),
                        services,
                        settled,
                        origins: origins(&held),
                    }
                }
                (None, Some(service)) => Reaches::ByName {
                    service: service.to_owned(),
                    why: wiring.why.clone().unwrap_or_default(),
                },
                _ => return None,
            };
            Some(Wired {
                by: wiring.by.clone(),
                reaches,
            })
        })
        .collect()
}

/// Every ask nothing fills, each naming what asked for it.
#[must_use]
pub fn unfilled(wired: &[Wired]) -> Vec<Unfilled> {
    wired
        .iter()
        .filter_map(|one| match &one.reaches {
            Reaches::Asked {
                capability,
                settled: Settled::Unfilled,
                ..
            } => Some(Unfilled {
                by: one.by.clone(),
                capability: capability.clone(),
            }),
            _ => None,
        })
        .collect()
}

/// Which service fills each capability the stack asks for, for whatever wires it.
///
/// A capability asked for by several links resolves the same way for all of them, so
/// the answer is one map rather than one per asker. Only what is settled appears: a
/// contest and an unfilled ask are both *nobody*, and a caller handed a name for
/// either would be wiring to a guess.
#[must_use]
pub fn filled(wired: &[Wired]) -> BTreeMap<String, Vec<String>> {
    let mut held: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for one in wired {
        if let Reaches::Asked {
            capability,
            services,
            ..
        } = &one.reaches
        {
            if !services.is_empty() {
                held.insert(capability.clone(), services.clone());
            }
        }
    }
    held
}

/// Every ask that installing `adding` would leave contested and that is not contested
/// now, answered as it would then stand.
///
/// The change an install makes to the wiring, which is a change like any other and is
/// stated before it happens: an ask one service answered becomes an ask nothing answers
/// until somebody chooses, and an operator finding that out from a service that stopped
/// reaching its library would have been told nothing by a rehearsal that said
/// everything else.
#[must_use]
pub fn contested_by(
    manifest: &Manifest,
    installed: &[Installed],
    adding: &Installed,
    chosen: &Chosen,
) -> Vec<Contest> {
    let already = contests(&settle(manifest, installed, chosen));
    let mut after = installed.to_vec();
    after.push(adding.clone());
    contests(&settle(manifest, &after, chosen))
        .into_iter()
        .filter(|one| {
            !already
                .iter()
                .any(|was| was.by == one.by && was.capability == one.capability)
        })
        .collect()
}

/// Every link that stands contested, as the contest it is.
fn contests(wired: &[Wired]) -> Vec<Contest> {
    wired
        .iter()
        .filter_map(|one| match &one.reaches {
            Reaches::Asked {
                capability,
                settled: Settled::Contested { claimants },
                ..
            } => Some(Contest {
                by: one.by.clone(),
                capability: capability.clone(),
                claimants: claimants.clone(),
            }),
            _ => None,
        })
        .collect()
}
