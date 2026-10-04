//! Who answers each of the stack's asks, in the terms something reaching it needs.
//!
//! The settlement above says *which* service fills an ask. Everything that then acts
//! on the answer — telling an \*arr about a download client, publishing a key, reading
//! what a filler holds — needs more than the name: where the filler answers beside the
//! others, which of lemonfiber's adapters it speaks, and where its credential is kept.
//! This is the one place that is worked out, for the stack's services and every
//! installed plugin's alike, so a plugin standing in for a bundled service is reached on
//! exactly the terms the service it replaced was.
//!
//! **Where a service answers is what it declares.** Its host is its own id on the
//! stack's network, except where it shares the network of a service granted the tunnel's
//! kernel capability: then the tunnel is the host, because that is whose address the
//! traffic goes to. The port is the one it says it listens on inside the network, never
//! the one it publishes on the host and never one this build's source names.

use std::path::Path;

use lemonfiber_manifest::{Api, ApiKind, Manifest, Service};

use crate::origin::Origin;
use crate::plugin::{Installed, Placed};

use super::{settle, Chosen, Reaches};

/// Where a service answers inside the stack's network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    /// The name the stack's network resolves to it.
    pub host: String,
    /// The port it answers on there.
    pub port: u16,
}

/// One service on this machine, the stack's or a plugin's, as whatever reaches it needs
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filler {
    /// The service's id, which is the name its container runs under.
    pub id: String,
    /// What it is called in front of an operator, and in the interface of whatever it
    /// is registered into.
    pub name: String,
    /// Whether this build's stack ships it or a named plugin brought it.
    pub origin: Origin,
    /// Where it answers beside the others, or nothing where it declares no port it
    /// listens on.
    pub address: Option<Address>,
    /// The adapter lemonfiber speaks to it through, where it names one.
    pub adapter: Option<Api>,
    /// The port this machine reaches it on, where it publishes one.
    pub published: Option<u16>,
    /// Where on this machine the file its credential is read from sits, where its
    /// adapter names one and the stack has been written to disk.
    pub key_file: Option<std::path::PathBuf>,
    /// The directory a plugin's container owns, which its credential file has to stay
    /// beneath when it is read; nothing for the stack's own services, whose images are
    /// the stack's.
    pub confined_to: Option<std::path::PathBuf>,
}

impl Filler {
    /// Whether lemonfiber speaks to it through this adapter.
    #[must_use]
    pub fn speaks(&self, kind: ApiKind) -> bool {
        self.adapter.as_ref().is_some_and(|api| api.kind == kind)
    }
}

/// One of the stack's asks, with every service that answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    /// The service that asks.
    pub by: String,
    /// What it asks for.
    pub capability: String,
    /// Whatever answers, as settled — empty where nothing fills it or a contest stands.
    pub fillers: Vec<Filler>,
}

/// Every service on this machine, and every ask the stack makes of them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fillers {
    /// The stack's services in declaration order, then each plugin's.
    services: Vec<Filler>,
    /// Each of the stack's asks, in the order the stack declares them.
    asks: Vec<Ask>,
}

impl Fillers {
    /// Every ask the stack declares answered against everything installed, and every
    /// service resolved to where it answers.
    ///
    /// `project` is where the stack was written to disk, which is what a credential file
    /// is found beneath; without one every service still has its address and its adapter
    /// and none has a file to read.
    #[must_use]
    pub fn of(
        manifest: &Manifest,
        installed: &[Installed],
        chosen: &Chosen,
        project: Option<&Path>,
    ) -> Self {
        let bundled = manifest
            .services
            .iter()
            .map(|service| bundled(service, &manifest.services, project));
        let brought = installed.iter().flat_map(|one| {
            one.services
                .iter()
                .map(|placed| brought(&one.plugin, placed, project))
        });
        let services: Vec<Filler> = bundled.chain(brought).collect();
        let asks = settle(manifest, installed, chosen)
            .into_iter()
            .filter_map(|wired| match wired.reaches {
                Reaches::Asked {
                    capability,
                    services: answering,
                    ..
                } => Some(Ask {
                    by: wired.by,
                    capability,
                    fillers: answering
                        .iter()
                        .filter_map(|id| services.iter().find(|one| one.id == *id).cloned())
                        .collect(),
                }),
                Reaches::ByName { .. } => None,
            })
            .collect();
        Self { services, asks }
    }

    /// Every ask the stack declares, in the order it declares them.
    #[must_use]
    pub fn asks(&self) -> &[Ask] {
        &self.asks
    }

    /// The service with this id, where there is one on this machine.
    #[must_use]
    pub fn service(&self, id: &str) -> Option<&Filler> {
        self.services.iter().find(|one| one.id == id)
    }

    /// Every service lemonfiber speaks to through this adapter, the stack's first.
    pub fn speaking(&self, kind: ApiKind) -> impl Iterator<Item = &Filler> {
        self.services.iter().filter(move |one| one.speaks(kind))
    }

    /// The setting a credential this service holds is kept under, ending in `holds`.
    ///
    /// A stack service's is named after its id. A plugin's is named in a namespace of its
    /// own, spelled so no two of its services' settings meet — and where one would still
    /// land on a setting this build names, or on one any other service here takes for
    /// any credential, the answer is nothing, so the setting is neither read for the
    /// plugin nor written for it. So is one ending in anything but a credential's ending,
    /// which is what the spelling's guarantee is made of.
    #[must_use]
    pub fn setting(&self, filler: &Filler, holds: &str) -> Option<String> {
        let setting = spelled(filler, holds);
        if !matches!(filler.origin, Origin::Plugin { .. }) {
            return Some(setting);
        }
        let credential = crate::config::CREDENTIAL_SUFFIXES.contains(&holds);
        let taken = self
            .services
            .iter()
            .filter(|one| !(one.id == filler.id && one.origin == filler.origin))
            .flat_map(|one| {
                crate::config::CREDENTIAL_SUFFIXES
                    .iter()
                    .map(|suffix| spelled(one, suffix))
            })
            .any(|other| other == setting);
        (credential && !taken && !crate::config::SETTINGS.contains(&setting.as_str()))
            .then_some(setting)
    }
}

/// The name a credential `filler` holds would be kept under, ending in `holds`, before
/// anything is refused.
fn spelled(filler: &Filler, holds: &str) -> String {
    match filler.origin {
        Origin::Plugin { .. } => crate::config::for_plugin(&filler.id, holds),
        _ => crate::config::for_service(&filler.id, holds),
    }
}

/// One of the stack's own services, as something reaching it needs it.
fn bundled(service: &Service, services: &[Service], project: Option<&Path>) -> Filler {
    Filler {
        id: service.id.clone(),
        name: service.name.clone(),
        origin: Origin::Bundled,
        address: service.listens.map(|port| Address {
            host: host(service, services),
            port,
        }),
        adapter: service.api.clone(),
        published: service.port,
        key_file: project.and_then(|project| {
            crate::app::targets::config_path(
                project,
                service,
                service.api.as_ref().and_then(|api| api.path.as_deref()),
            )
        }),
        confined_to: None,
    }
}

/// One installed plugin's service, as something reaching it needs it.
///
/// Reached at its own id: a plugin's service has no way to share another container's
/// network, so there is no tunnel in front of it to be reached through.
fn brought(plugin: &str, placed: &Placed, project: Option<&Path>) -> Filler {
    Filler {
        id: placed.service.clone(),
        name: placed.called().to_owned(),
        origin: Origin::Plugin {
            named: plugin.to_owned(),
        },
        address: placed.listens.map(|port| Address {
            host: placed.service.clone(),
            port,
        }),
        adapter: placed.api.clone(),
        published: placed.published(),
        key_file: project
            .and_then(|project| crate::app::targets::plugin_config_path(project, placed)),
        confined_to: project
            .and_then(|project| crate::app::targets::plugin_config_dir(project, placed)),
    }
}

/// The name the stack's network resolves a bundled service to: its own id, or the id of
/// the tunnel whose network it shares.
///
/// The tunnel is recognised by the kernel capability it is granted rather than by name,
/// so a stack whose tunnel is called something else is read the same way.
fn host(service: &Service, services: &[Service]) -> String {
    for on in &service.depends_on {
        let tunnel = services.iter().find(|other| {
            other.id == *on
                && other
                    .grants
                    .iter()
                    .any(|granted| granted == crate::doctor::vpn::GATEWAY_GRANT)
        });
        if let Some(tunnel) = tunnel {
            return tunnel.id.clone();
        }
    }
    service.id.clone()
}

#[cfg(test)]
mod tests;
