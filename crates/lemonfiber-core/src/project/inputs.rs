//! What the project's networks are derived from: every service it writes, every ask
//! settled between them, every link by name, and the service that guards the household's
//! streams.

use lemonfiber_contract::capabilities::proxy::front;
use lemonfiber_contract::capabilities::stream::guard;
use lemonfiber_manifest::container::{Posture, Value};
use lemonfiber_manifest::{Manifest, Service};

use crate::plugin::{Installed, Placed, Reached};
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
}

/// Everything the derivation reads, owned.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Inputs {
    /// Every service the project writes.
    pub(super) members: Vec<Standing>,
    /// Every settled ask, as asker, capability and the service it reaches.
    pub(super) asks: Vec<(String, String, String)>,
    /// Every link by name, as the service that reaches and the one it names.
    pub(super) links: Vec<(String, String)>,
    /// The service that fills `stream.guard`, where one does.
    pub(super) guard: Option<String>,
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
            .map(|(asker, capability, reached)| Settled {
                asker,
                capability,
                reached,
            })
            .collect();
        let links: Vec<Named<'_>> = self.links.iter().map(|(by, to)| Named { by, to }).collect();
        topology::derived(&members, &asks, &links, self.guard.as_deref())
    }
}

/// The inputs `manifest`'s services and `installed`'s, wired as `chosen` settles them,
/// come to.
pub(super) fn read(manifest: &Manifest, installed: &[Installed], chosen: &Chosen) -> Inputs {
    let fill = |capability: &str| filler(manifest, installed, chosen, capability);
    let mut members: Vec<Standing> = written(manifest, installed)
        .map(|service| Standing {
            id: service.id.clone(),
            confined: service.posture == Posture::Confined,
            publishes: service.port.is_some() || !service.publish.is_empty(),
            inside: service
                .network
                .as_ref()
                .and_then(|network| fill(&network.via)),
            fronts: None,
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
                    inside: placed
                        .fields
                        .network
                        .as_ref()
                        .and_then(|network| fill(&network.via)),
                    fronts: placed.fronts.clone(),
                }),
        )
        .collect();
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
    let mut asks = Vec::new();
    let mut links = Vec::new();
    for wired in settle(manifest, installed, chosen) {
        match wired.reaches {
            Reaches::Asked {
                capability,
                services,
                ..
            } => asks.extend(
                services
                    .into_iter()
                    .map(|reached| (wired.by.clone(), capability.clone(), reached)),
            ),
            Reaches::ByName { service, .. } => links.push((wired.by, service)),
        }
    }
    asks.extend(narrowed(manifest, installed));
    asks.extend(fronted(installed, fill(front::CAPABILITY)));
    Inputs {
        members,
        asks,
        links,
        guard: fill(guard::CAPABILITY),
    }
}

/// The ask of `proxy.front` each plugin service the household reaches at a hostname makes
/// of `proxy`, the service that fills it.
fn fronted(installed: &[Installed], proxy: Option<String>) -> Vec<(String, String, String)> {
    let Some(proxy) = proxy else {
        return Vec::new();
    };
    installed
        .iter()
        .flat_map(|one| &one.services)
        .filter(|placed| matches!(placed.reached, Some(Reached::Household { .. })))
        .map(|placed| {
            (
                placed.service.clone(),
                front::CAPABILITY.to_owned(),
                proxy.clone(),
            )
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
    declared(manifest).filter(move |service| {
        !installed
            .iter()
            .flat_map(|one| &one.services)
            .any(|placed| placed.replaces.as_deref() == Some(service.id.as_str()))
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

/// The asks a service's addresses make that settling does not: one narrowed to a medium,
/// which reaches every claimant filing it, and one narrowed to an API, which reaches the
/// upstream an adapter claiming it fronts.
fn narrowed(manifest: &Manifest, installed: &[Installed]) -> Vec<(String, String, String)> {
    let stack =
        written(manifest, installed).map(|service| (service.id.as_str(), service.env.values()));
    let plugins = installed
        .iter()
        .flat_map(|one| &one.services)
        .map(|placed| (placed.service.as_str(), placed.fields.env.values()));
    let mut asks = Vec::new();
    for (by, values) in stack.chain(plugins) {
        for value in values {
            let Value::Url(url) = value else { continue };
            if url.files.is_none() && url.native.is_none() {
                continue;
            }
            let native = url.native.is_some();
            for reached in reaching(
                manifest,
                installed,
                &url.url_of,
                url.files.as_deref(),
                native,
            ) {
                if reached != by {
                    asks.push((by.to_owned(), url.url_of.clone(), reached));
                }
            }
        }
    }
    asks
}

/// Every service an ask for `capability`, narrowed to `files` where it names a medium,
/// reaches: each claimant filing it, and the upstream an adapter fronts where the ask
/// speaks the upstream's own API.
pub(super) fn reaching(
    manifest: &Manifest,
    installed: &[Installed],
    capability: &str,
    files: Option<&str>,
    native: bool,
) -> Vec<String> {
    let files_it = |media: &[String]| files.is_none_or(|medium| media.iter().any(|m| m == medium));
    written(manifest, installed)
        .filter(|service| claims(&service.provides, capability) && files_it(&service.media_types))
        .map(|service| service.id.clone())
        .chain(
            installed
                .iter()
                .flat_map(|one| &one.services)
                .filter(|placed: &&Placed| {
                    claims(&placed.provides, capability) && files_it(&placed.media_types)
                })
                .map(|placed| match (&placed.fronts, native) {
                    (Some(upstream), true) => upstream.clone(),
                    _ => placed.service.clone(),
                }),
        )
        .collect()
}
