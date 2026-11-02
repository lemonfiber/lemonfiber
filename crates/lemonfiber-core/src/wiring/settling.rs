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
use crate::plugin::first_party::FirstParty;
use crate::plugin::{Asking, Installed, Placed};

use super::{Chosen, Contest, Reaches, Settled, Unfilled, Whose, Wired};

/// Everything installed that claims a capability: the stack's services in declaration
/// order, but one an installed service has moved in place of, then each installed
/// plugin's in the order the record holds them.
///
/// `Claimed` for both. A declaration is what the manifest holds and running a probe is
/// a separate act; a plugin's declaration has already been held to its demonstrations
/// before it could be installed, and the stack's is read as it always was.
pub(super) fn claimants(
    manifest: &Manifest,
    installed: &[Installed],
    (capability, files): (&str, Option<&str>),
    trusted: &[FirstParty],
) -> Vec<Claimant> {
    let filing =
        |media: &[String]| files.is_none_or(|medium| media.iter().any(|one| one == medium));
    let bundled = manifest
        .services
        .iter()
        .filter(|service| service.provides.iter().any(|named| named == capability))
        .filter(|service| filing(&service.media_types))
        .filter(|service| crate::project::moving(installed, &service.id, trusted).is_none())
        .map(|service| Claimant {
            service: service.id.clone(),
            plugin: None,
            shown: Shown::Claimed,
        });
    let brought = installed.iter().flat_map(|one| {
        one.services
            .iter()
            .filter(|placed| placed.provides.iter().any(|named| named == capability))
            .filter(|placed| filing(&placed.media_types))
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

/// What one ask comes to, given its claimants and whatever has been chosen among
/// everything `installed`, `trusted` holding the plugins that are first-party.
fn asked(
    wiring: &Wiring,
    capability: &str,
    held: &[Claimant],
    chosen: &Chosen,
    (installed, trusted): (&[Installed], &[FirstParty]),
) -> (Vec<String>, Settled) {
    let names: Vec<String> = held.iter().map(|one| one.service.clone()).collect();
    if wiring.each && !names.is_empty() {
        return (names, Settled::Each);
    }

    let picked = chosen
        .filler(capability)
        .map(|service| {
            (
                service,
                Whose::Operator,
                chosen.why(capability).map(str::to_owned),
            )
        })
        .or_else(|| {
            wiring
                .filled_by
                .as_deref()
                .map(|service| (service, Whose::Stack, wiring.why.clone()))
        })
        .filter(|_| names.len() > 1)
        .and_then(|(service, whose, why)| {
            answering(installed, trusted, &names, service).map(|service| (service, whose, why))
        });

    match picked {
        Some((service, whose, why)) => (
            vec![service.clone()],
            Settled::Chosen {
                whose,
                why,
                over: names
                    .iter()
                    .filter(|one| **one != service)
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

/// The claimant among `names` answering for the service `chose` names: that service where
/// it claims, and otherwise the adapter in front of it that the first-party plugin moved
/// in its place brings, which stands where it stood. Another plugin's service fronting
/// it is never credited with the choice.
fn answering(
    installed: &[Installed],
    trusted: &[FirstParty],
    names: &[String],
    chose: &str,
) -> Option<String> {
    if names.iter().any(|one| one == chose) {
        return Some(chose.to_owned());
    }
    crate::project::moving(installed, chose, trusted)?
        .services
        .iter()
        .filter(|placed| placed.fronts.as_deref() == Some(chose))
        .map(|placed| placed.service.clone())
        .find(|adapter| names.contains(adapter))
}

/// Every link the stack declares, then every ask each installed plugin's services make,
/// answered against everything installed that claims what it asks.
///
/// An ask the stack declares for a service a first-party part has moved in place of is
/// left out: the part's own service asks for itself, by its plugin's asks, and each of
/// those is settled by the stack's choice for that capability as every plugin's ask is.
///
/// A link naming a `why` it should not have, or asking for something of the wrong
/// shape, is the validator's business rather than this one's: what is read here is a
/// manifest that has already been checked, and a reader that second-guessed it would
/// be a second opinion on the same file.
#[must_use]
pub fn settle(manifest: &Manifest, installed: &[Installed], chosen: &Chosen) -> Vec<Wired> {
    settle_trusting(
        manifest,
        installed,
        chosen,
        crate::plugin::first_party::EMBEDDED,
    )
}

/// As [`settle`], with `trusted` as the plugins that are first-party.
#[must_use]
pub fn settle_trusting(
    manifest: &Manifest,
    installed: &[Installed],
    chosen: &Chosen,
    trusted: &[FirstParty],
) -> Vec<Wired> {
    let stack = manifest.wirings.iter().filter_map(|wiring| {
        let reaches = match (wiring.asks.as_deref(), wiring.to.as_deref()) {
            (Some(_), None) if crate::project::moving(installed, &wiring.by, trusted).is_some() => {
                return None;
            }
            (Some(capability), None) => reached(
                manifest,
                (installed, trusted),
                wiring,
                &stack_ask(capability),
                chosen,
            ),
            (None, Some(service)) => Reaches::ByName {
                service: service.to_owned(),
                why: wiring.why.clone().unwrap_or_default(),
            },
            _ => return None,
        };
        Some(Wired {
            by: wiring.by.clone(),
            origin: Origin::Bundled,
            reaches,
        })
    });
    let implied =
        crate::project::written_trusting(manifest, installed, trusted).flat_map(|service| {
            crate::plugin::implied(&service.env, service.network.as_ref())
                .into_iter()
                .filter(|ask| {
                    !manifest.wirings.iter().any(|wiring| {
                        wiring.by == service.id
                            && wiring.asks.as_deref() == Some(ask.capability.as_str())
                            && ask.files.is_none()
                    })
                })
                .map(|ask| Wired {
                    by: service.id.clone(),
                    origin: Origin::Bundled,
                    reaches: reached(
                        manifest,
                        (installed, trusted),
                        &plugins_ask(manifest, &service.id, &ask),
                        &ask,
                        chosen,
                    ),
                })
        });
    let brought = installed.iter().flat_map(|one| {
        one.services.iter().flat_map(move |placed| {
            placed.asking().into_iter().map(move |ask| {
                let wiring = plugins_ask(manifest, &placed.service, &ask);
                Wired {
                    by: placed.service.clone(),
                    origin: Origin::Plugin {
                        named: one.plugin.clone(),
                    },
                    reaches: reached(manifest, (installed, trusted), &wiring, &ask, chosen),
                }
            })
        })
    });
    stack.chain(implied).chain(brought).collect()
}

/// A link of the stack's asking for `capability`, narrowed by nothing.
fn stack_ask(capability: &str) -> Asking {
    Asking {
        capability: capability.to_owned(),
        each: false,
        native: None,
        files: None,
    }
}

/// What `ask` reaches, against everything installed that claims its capability and files
/// the medium it narrows to, talking in the upstream's own API where it names one.
fn reached(
    manifest: &Manifest,
    (installed, trusted): (&[Installed], &[FirstParty]),
    wiring: &Wiring,
    ask: &Asking,
    chosen: &Chosen,
) -> Reaches {
    let capability = ask.capability.as_str();
    let held = claimants(
        manifest,
        installed,
        (capability, ask.files.as_deref()),
        trusted,
    );
    let (services, settled) = asked(wiring, capability, &held, chosen, (installed, trusted));
    let (services, settled) = match ask.native.as_deref() {
        Some(api) => in_native(installed, &services, settled, api),
        None => (services, settled),
    };
    Reaches::Asked {
        capability: capability.to_owned(),
        services,
        settled,
        origins: origins(&held),
        native: ask.native.clone(),
        files: ask.files.clone(),
    }
}

/// What an ask in the API `api` settles on: the upstream each adapter in `services`
/// fronts, where that upstream declares `api`, each other service as itself, where it
/// declares `api` or no API of its own, and unfilled where none is left.
fn in_native(
    installed: &[Installed],
    services: &[String],
    settled: Settled,
    api: &str,
) -> (Vec<String>, Settled) {
    let spoken: Vec<String> = services
        .iter()
        .filter_map(|service| spoken_by(installed, service, api))
        .collect();
    if spoken.is_empty() && !services.is_empty() {
        (spoken, Settled::Unfilled)
    } else {
        (spoken, settled)
    }
}

/// What of `service` an ask in the API `api` reaches: the upstream it fronts where it is
/// an adapter and that upstream declares `api`, and `service` itself where it fronts
/// nothing and declares `api` or no API of its own.
fn spoken_by(installed: &[Installed], service: &str, api: &str) -> Option<String> {
    let speaks = |placed: &Placed| placed.native.as_deref().is_none_or(|own| own == api);
    let Some((one, placed)) = installed.iter().find_map(|one| {
        one.services
            .iter()
            .find(|placed| placed.service == service)
            .map(|placed| (one, placed))
    }) else {
        return Some(service.to_owned());
    };
    match placed.fronts.as_deref() {
        Some(upstream) => one
            .services
            .iter()
            .find(|placed| placed.service == upstream && placed.native.as_deref() == Some(api))
            .map(|placed| placed.service.clone()),
        None => speaks(placed).then(|| service.to_owned()),
    }
}

/// A plugin's ask as a link of the stack's, carrying the stack's own choice for that
/// capability.
fn plugins_ask(manifest: &Manifest, by: &str, ask: &Asking) -> Wiring {
    let stacks = manifest.wirings.iter().find(|wiring| {
        wiring.asks.as_deref() == Some(ask.capability.as_str()) && wiring.filled_by.is_some()
    });
    Wiring {
        by: by.to_owned(),
        asks: Some(ask.capability.clone()),
        each: ask.each,
        filled_by: stacks.and_then(|wiring| wiring.filled_by.clone()),
        to: None,
        why: stacks.and_then(|wiring| wiring.why.clone()),
    }
}

/// Every ask `adding`'s services would make once it is installed beside `installed`, as
/// each would then stand.
#[must_use]
pub fn asked_by(
    manifest: &Manifest,
    installed: &[Installed],
    adding: &Installed,
    chosen: &Chosen,
) -> Vec<Wired> {
    let mut after = installed.to_vec();
    after.push(adding.clone());
    settle(manifest, &after, chosen)
        .into_iter()
        .filter(
            |wired| matches!(&wired.origin, Origin::Plugin { named } if *named == adding.plugin),
        )
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
                native,
                files,
                ..
            } => Some(Unfilled {
                by: one.by.clone(),
                capability: capability.clone(),
                native: native.clone(),
                files: files.clone(),
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
