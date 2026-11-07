//! What the project's networks are derived from: every service it writes, every ask
//! settled between them, every link by name, and the service that guards the household's
//! streams.

use lemonfiber_contract::capabilities::library::curate;
use lemonfiber_contract::capabilities::proxy::front;
use lemonfiber_contract::capabilities::request::gate as request_gate;
use lemonfiber_contract::capabilities::stream::guard;
use lemonfiber_manifest::container::{Network, Posture};
use lemonfiber_manifest::{Manifest, Service};

use crate::plugin::{Installed, Reached};
use crate::topology::{self, Member, Named, Settled, Topology};
use crate::wiring::{settle, Chosen, Reaches};

/// One service, as its place on the networks needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Standing {
    /// Its id.
    pub(super) id: String,
    /// Whether it is confined.
    pub(super) confined: bool,
    /// Whether it publishes a port of its own.
    pub(super) publishes: bool,
    /// The service whose network it runs inside, where it takes `via`.
    pub(super) inside: Option<String>,
    /// The upstream it fronts, where it is an adapter.
    pub(super) fronts: Option<String>,
    /// Whether it declares itself fenced.
    pub(super) fenced: bool,
}

/// One settled ask: what asked, the capability and the medium it asked of, and the service
/// it reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Ask {
    /// The service that asked.
    pub(super) asker: String,
    /// The capability it asked for.
    pub(super) capability: String,
    /// The medium it narrowed the ask to, where it named one.
    pub(super) files: Option<String>,
    /// The service the ask reaches.
    pub(super) reached: String,
}

/// Everything the derivation reads, owned.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Inputs {
    /// Every service the project writes.
    pub(super) members: Vec<Standing>,
    /// Every settled ask a service on the networks makes.
    pub(super) asks: Vec<Ask>,
    /// Every link by name, as the service that reaches and the one it names.
    pub(super) links: Vec<(String, String)>,
    /// The service that fills `stream.guard`, where one does.
    pub(super) guard: Option<String>,
    /// Whether the pass service runs beside the guard.
    pub(super) pass: bool,
    /// Each service declaring itself fenced, with the fence it runs inside, or none where
    /// another service already holds that fence's id.
    pub(super) fenced: Vec<(String, Option<String>)>,
}

impl Inputs {
    /// The networks these inputs derive.
    pub(super) fn topology(&self) -> Topology {
        let members: Vec<Member<'_>> = self
            .members
            .iter()
            .map(|standing| Member {
                id: &standing.id,
                confined: standing.confined,
                publishes: standing.publishes,
                inside: standing.inside.as_deref(),
                fronts: standing.fronts.as_deref(),
            })
            .collect();
        let asks: Vec<Settled<'_>> = self
            .asks
            .iter()
            .map(|ask| Settled {
                asker: &ask.asker,
                capability: &ask.capability,
                reached: &ask.reached,
            })
            .collect();
        let links: Vec<Named<'_>> = self.links.iter().map(|(by, to)| Named { by, to }).collect();
        topology::derived(&members, &asks, &links, self.guard.as_deref(), self.pass)
    }
}

/// The inputs `manifest`'s services and `installed`'s, wired as `chosen` settles them,
/// come to, with the pass service among them where `pass` is pinned and a guard is filled.
pub(super) fn read(
    manifest: &Manifest,
    installed: &[Installed],
    chosen: &Chosen,
    pass: bool,
) -> Inputs {
    let fill = |capability: &str| filler(manifest, installed, chosen, capability);
    let settled = settle(manifest, installed, chosen);
    let vias = vias(manifest, installed);
    let inside = |id: &str| inside(installed, &settled, &vias, id);
    let mut members: Vec<Standing> = written(manifest, installed)
        .map(|service| Standing {
            id: service.id.clone(),
            confined: service.posture == Posture::Confined,
            publishes: service.port.is_some() || !service.publish.is_empty(),
            inside: inside(&service.id),
            fronts: None,
            fenced: service.fenced(),
        })
        .chain(
            installed
                .iter()
                .flat_map(|one| &one.services)
                .map(|placed| Standing {
                    id: placed.service.clone(),
                    confined: placed.fields.posture == Posture::Confined,
                    publishes: placed.reached.is_some()
                        || !placed.fields.publish.is_empty()
                        || (placed.listens.is_some() && !placed.speaks.is_empty()),
                    inside: inside(&placed.service),
                    fronts: placed.fronts.clone(),
                    fenced: placed
                        .fields
                        .network
                        .as_ref()
                        .is_some_and(|network| network.fenced),
                }),
        )
        .collect();
    let fenced = fences(&mut members);
    let hosting: Vec<String> = members
        .iter()
        .filter(|standing| standing.publishes)
        .filter_map(|standing| standing.inside.clone())
        .collect();
    for standing in &mut members {
        if hosting.contains(&standing.id) {
            standing.publishes = true;
        }
    }
    let guard = fill(guard::CAPABILITY);
    let pass = pass && guard.is_some() && !pass_taken(manifest, installed);
    if pass {
        members.push(Standing {
            id: crate::pass::SERVICE.to_owned(),
            confined: true,
            publishes: false,
            inside: None,
            fronts: None,
            fenced: false,
        });
    }
    let (mut asks, links) = joined(settled, &vias);
    asks.extend(fronted(installed, fill(front::CAPABILITY)));
    Inputs {
        members,
        asks,
        links,
        guard,
        pass,
        fenced,
    }
}

/// Each service of the stack and of `installed` that runs inside the network of the filler
/// of a capability, with that capability.
fn vias<'a>(manifest: &'a Manifest, installed: &'a [Installed]) -> Vec<(&'a str, &'a str)> {
    written(manifest, installed)
        .filter_map(|service| {
            let via = service.network.as_ref().and_then(Network::via)?;
            Some((service.id.as_str(), via))
        })
        .chain(
            installed
                .iter()
                .flat_map(|one| &one.services)
                .filter_map(|placed| {
                    let via = placed.fields.network.as_ref().and_then(Network::via)?;
                    Some((placed.service.as_str(), via))
                }),
        )
        .collect()
}

/// Where `id` runs inside, as `vias` says it does: the one service its ask of that
/// capability settled on, or the upstream that service fronts where it is an adapter.
fn inside(
    installed: &[Installed],
    settled: &[crate::wiring::Wired],
    vias: &[(&str, &str)],
    id: &str,
) -> Option<String> {
    let (_, via) = vias.iter().find(|(service, _)| *service == id)?;
    settled.iter().find_map(|wired| match &wired.reaches {
        Reaches::Asked {
            capability,
            services,
            ..
        } if wired.by == id && capability == via => match services.as_slice() {
            [one] => Some(upstream_of(installed, one)),
            _ => None,
        },
        _ => None,
    })
}

/// Each settled ask as a network its asker joins, but the ask of the capability a
/// service runs inside the network of, a curator asked of by a service the request gate
/// stands in front of, which has no other path to it, and each link by name.
fn joined(
    settled: Vec<crate::wiring::Wired>,
    vias: &[(&str, &str)],
) -> (Vec<Ask>, Vec<(String, String)>) {
    let gated: Vec<String> = settled
        .iter()
        .filter(|wired| {
            matches!(
                &wired.reaches,
                Reaches::Asked { capability, services, .. }
                    if capability == request_gate::CAPABILITY && !services.is_empty()
            )
        })
        .map(|wired| wired.by.clone())
        .collect();
    let mut asks = Vec::new();
    let mut links = Vec::new();
    for wired in settled {
        match wired.reaches {
            Reaches::Asked { capability, .. }
                if capability == curate::CAPABILITY && gated.contains(&wired.by) => {}
            Reaches::Asked {
                capability,
                services,
                files,
                ..
            } if !vias.contains(&(wired.by.as_str(), capability.as_str())) => {
                asks.extend(services.into_iter().map(|reached| Ask {
                    asker: wired.by.clone(),
                    capability: capability.clone(),
                    files: files.clone(),
                    reached,
                }));
            }
            Reaches::Asked { .. } => {}
            Reaches::ByName { service, .. } => links.push((wired.by, service)),
        }
    }
    (asks, links)
}

/// A fence beside each of `members` that declares one, holding the networks and the
/// ports it would have held, with the member placed inside it: each such member with its
/// fence, or with none where another member already holds the fence's id.
fn fences(members: &mut Vec<Standing>) -> Vec<(String, Option<String>)> {
    let ids: std::collections::BTreeSet<String> =
        members.iter().map(|standing| standing.id.clone()).collect();
    let mut fenced = Vec::new();
    let mut written = Vec::new();
    for standing in members.iter_mut().filter(|standing| standing.fenced) {
        let fence = crate::fence::of(&standing.id);
        if ids.contains(&fence) {
            fenced.push((standing.id.clone(), None));
            continue;
        }
        written.push(Standing {
            id: fence.clone(),
            confined: standing.confined,
            publishes: false,
            inside: None,
            fronts: standing.fronts.clone(),
            fenced: false,
        });
        standing.inside = Some(fence.clone());
        fenced.push((standing.id.clone(), Some(fence)));
    }
    members.extend(written);
    fenced
}

/// Whether a service of the stack or of a plugin holds the id the pass service runs
/// under, which leaves the pass service out rather than written over it.
pub(crate) fn pass_taken(manifest: &Manifest, installed: &[Installed]) -> bool {
    written(manifest, installed)
        .map(|service| service.id.as_str())
        .chain(
            installed
                .iter()
                .flat_map(|one| &one.services)
                .map(|placed| placed.service.as_str()),
        )
        .any(|id| id == crate::pass::SERVICE)
}

/// The ask of `proxy.front` each plugin service the household reaches at a hostname makes
/// of `proxy`, the service that fills it.
fn fronted(installed: &[Installed], proxy: Option<String>) -> Vec<Ask> {
    let Some(proxy) = proxy else {
        return Vec::new();
    };
    installed
        .iter()
        .flat_map(|one| &one.services)
        .filter(|placed| matches!(placed.reached, Some(Reached::Household { .. })))
        .map(|placed| Ask {
            asker: placed.service.clone(),
            capability: front::CAPABILITY.to_owned(),
            files: None,
            reached: proxy.clone(),
        })
        .collect()
}

/// Every stack service lemonfiber runs: all but those the host's own service manager
/// runs.
pub(crate) fn declared(manifest: &Manifest) -> impl Iterator<Item = &Service> {
    manifest
        .services
        .iter()
        .filter(|service| !service.host_managed)
}

/// Every stack service the project writes an entry for: each lemonfiber runs but one an
/// installed service has moved in place of.
pub(crate) fn written<'a>(
    manifest: &'a Manifest,
    installed: &'a [Installed],
) -> impl Iterator<Item = &'a Service> {
    written_trusting(manifest, installed, crate::plugin::first_party::EMBEDDED)
}

/// The same, with `trusted` as the plugins that are first-party.
pub(crate) fn written_trusting<'a>(
    manifest: &'a Manifest,
    installed: &'a [Installed],
    trusted: &'a [crate::plugin::first_party::FirstParty],
) -> impl Iterator<Item = &'a Service> {
    declared(manifest).filter(move |service| moving(installed, &service.id, trusted).is_none())
}

/// Whether one of `installed` has moved in place of the stack's service `service`: a
/// first-party plugin's service taking its id, the one kind of service that stands in
/// for the stack's, so no other plugin can put the stack's service out by naming it.
pub(crate) fn moved_in_place(installed: &[Installed], service: &str) -> bool {
    moving(installed, service, crate::plugin::first_party::EMBEDDED).is_some()
}

/// The plugin of `installed` that has moved in place of the stack's service `service`:
/// one `trusted` holds as first-party, whose own service takes that id in its place.
pub(crate) fn moving<'a>(
    installed: &'a [Installed],
    service: &str,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> Option<&'a Installed> {
    installed
        .iter()
        .filter(|one| crate::plugin::first_party::holds(trusted, one))
        .find(|one| {
            one.services.iter().any(|placed| {
                placed.service == service && placed.replaces.as_deref() == Some(service)
            })
        })
}

/// The one service that fills `capability`: the operator's choice where it names a
/// service that claims it, and the only claimant otherwise. An adapter's place is its
/// upstream's, so a capability an adapter fills is held by the service it fronts.
pub(crate) fn filler(
    manifest: &Manifest,
    installed: &[Installed],
    chosen: &Chosen,
    capability: &str,
) -> Option<String> {
    let claimants: Vec<(String, Option<String>)> = written(manifest, installed)
        .filter(|service| claims(&service.provides, capability))
        .map(|service| (service.id.clone(), None))
        .chain(
            installed
                .iter()
                .flat_map(|one| &one.services)
                .filter(|placed| claims(&placed.provides, capability))
                .map(|placed| (placed.service.clone(), placed.fronts.clone())),
        )
        .collect();
    let picked = match chosen.filler(capability) {
        Some(service) => claimants.iter().find(|(id, _)| id == service),
        None => match claimants.as_slice() {
            [one] => Some(one),
            _ => None,
        },
    }?;
    Some(picked.1.clone().unwrap_or_else(|| picked.0.clone()))
}

/// Whether a service's list of what it provides names `capability`.
fn claims(provides: &[String], capability: &str) -> bool {
    provides.iter().any(|named| named == capability)
}

/// Where an ask settled on `service` is held: the upstream `service` fronts where it is an
/// adapter, and `service` itself otherwise.
fn upstream_of(installed: &[Installed], service: &str) -> String {
    installed
        .iter()
        .flat_map(|one| &one.services)
        .find(|placed| placed.service == service)
        .and_then(|placed| placed.fronts.clone())
        .unwrap_or_else(|| service.to_owned())
}

#[cfg(test)]
mod tests;
