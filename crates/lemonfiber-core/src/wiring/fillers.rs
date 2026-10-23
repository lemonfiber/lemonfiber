//! Who answers each of the stack's asks, in the terms something reaching it needs.
//!
//! The settlement above says *which* service fills an ask. Everything that then acts
//! on the answer — telling a curator about a download client, publishing a key, reading
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

impl Address {
    /// Where a service beside it reaches it over HTTP.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
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
    /// The directory its container owns, which its credential file has to stay beneath
    /// when it is read, where the stack has been written to disk.
    ///
    /// Every service's, the stack's own included: whatever runs in a container can write
    /// the directory mounted into it, whoever published the image.
    pub confined_to: Option<std::path::PathBuf>,
    /// The media it files, which decides which of an asker's connections it comes to.
    pub media_types: Vec<String>,
    /// Every capability it says it provides.
    pub provides: Vec<String>,
    /// Every contract it is asked over, each `capability@major`.
    pub contracts: Vec<String>,
    /// Whether the plugin that brought it is one this build embeds as first-party.
    pub first_party: bool,
    /// The setting of each credential of another service the operator granted the plugin
    /// that brought it (`F14-R20`).
    pub granted: Vec<String>,
    /// The majors of its image that run here: the first number of the tag it is pinned
    /// by, or nothing where the tag does not open with one.
    pub majors: Vec<u32>,
    /// The service of the same plugin it stands in front of, where it is an adapter.
    pub fronts: Option<String>,
    /// The API it answers the stack's other services in, where it names one.
    pub native: Option<String>,
    /// The id of the stack's service it takes the place of, where it names one.
    pub replaces: Option<String>,
    /// The capability whose filler's network it runs inside, where it takes `network.via`.
    pub via: Option<String>,
}

impl Filler {
    /// The service as one of the curator shape this machine can open: where it reaches
    /// it, the file its key is in and the version of the shape it speaks, or nothing
    /// where it speaks another, publishes no port or names no file.
    #[must_use]
    pub fn target(&self) -> Option<crate::doctor::credentials::Target> {
        let api = self
            .adapter
            .as_ref()
            .filter(|api| api.kind == ApiKind::Servarr)?;
        Some(crate::doctor::credentials::Target {
            id: self.id.clone(),
            name: self.name.clone(),
            base: crate::app::targets::loopback(self.published?),
            config: self.key_file.clone()?,
            version: api.version?,
            confined_to: self.confined_to.clone(),
            kind: crate::ports::media::Kind::of_declared(&self.media_types),
        })
    }

    /// Whether it is a first-party plugin's service taking the place of the stack's
    /// service of its own id, which keeps that service's names and links.
    #[must_use]
    pub fn stands_in(&self) -> bool {
        self.first_party && self.replaces.as_deref() == Some(self.id.as_str())
    }

    /// Who holds what it holds, as a credential crossing to or from it is judged.
    #[must_use]
    pub fn holder(&self) -> Holder<'_> {
        match &self.origin {
            Origin::Bundled => Holder::Stack,
            Origin::Plugin { named } if self.first_party => Holder::FirstParty(named),
            Origin::Plugin { named } => Holder::ThirdParty(named),
            _ => Holder::Nobody,
        }
    }

    /// The plugin that brought it, or nothing where the stack ships it.
    #[must_use]
    pub fn brought_by(&self) -> Option<&str> {
        match &self.origin {
            Origin::Plugin { named } => Some(named),
            _ => None,
        }
    }

    /// Whether it is asked over `capability`'s contract at `major`.
    #[must_use]
    pub fn contracted(&self, capability: &str, major: u32) -> bool {
        self.contracts
            .contains(&lemonfiber_contract::spoken(capability, major))
    }

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

/// Every service on this machine, and every ask the stack and its plugins make of them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fillers {
    /// The stack's services in declaration order, then each plugin's.
    services: Vec<Filler>,
    /// Each ask, the stack's in the order it declares them and then each plugin's.
    asks: Vec<Ask>,
    /// Each link the stack makes by name, as the service it runs from and the one it names.
    named: Vec<(String, String)>,
}

impl Fillers {
    /// Every ask the stack and its plugins make, answered against everything installed,
    /// and every service resolved to where it answers.
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
        Self::trusting(
            manifest,
            installed,
            chosen,
            project,
            crate::plugin::first_party::EMBEDDED,
        )
    }

    /// As [`Self::of`], with `trusted` as the plugins that are first-party.
    #[must_use]
    pub fn trusting(
        manifest: &Manifest,
        installed: &[Installed],
        chosen: &Chosen,
        project: Option<&Path>,
        trusted: &[crate::plugin::first_party::FirstParty],
    ) -> Self {
        let bundled = manifest
            .services
            .iter()
            .map(|service| bundled(service, &manifest.services, project));
        let brought = installed.iter().flat_map(|one| {
            let first_party = crate::plugin::first_party::holds(trusted, one);
            one.services.iter().map(move |placed| Filler {
                granted: one
                    .granted
                    .iter()
                    .map(|grant| grant.credential.clone())
                    .collect(),
                ..brought(&one.plugin, placed, project, first_party)
            })
        });
        let mut services: Vec<Filler> = bundled.chain(brought).collect();
        for one in services
            .iter_mut()
            .filter(|one| one.origin != Origin::Bundled)
        {
            let guard = one
                .via
                .as_deref()
                .and_then(|via| crate::project::placed_at(manifest, installed, chosen, via));
            if let (Some(address), Some(guard)) = (one.address.as_mut(), guard) {
                address.host = guard;
            }
        }
        let stood_in: Vec<String> = services
            .iter()
            .filter(|one| one.stands_in())
            .map(|one| one.id.clone())
            .collect();
        services.retain(|one| !(one.origin == Origin::Bundled && stood_in.contains(&one.id)));
        let settled = settle(manifest, installed, chosen);
        let named = settled
            .iter()
            .filter_map(|wired| match &wired.reaches {
                Reaches::ByName { service, .. } => Some((wired.by.clone(), service.clone())),
                Reaches::Asked { .. } => None,
            })
            .collect();
        let asks = settled
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
        Self {
            services,
            asks,
            named,
        }
    }

    /// The stack's own service that `by` is linked to by name, where the stack declares
    /// such a link and runs that service, or the first-party service that stands in for
    /// it: never another plugin's, whatever it stands in for.
    #[must_use]
    pub fn named_by(&self, by: &str) -> Option<&Filler> {
        let (_, to) = self.named.iter().find(|(from, _)| from == by)?;
        self.service(to)
            .filter(|filler| matches!(filler.origin, Origin::Bundled) || filler.stands_in())
    }

    /// Every ask, the stack's in the order it declares them and then each plugin's.
    #[must_use]
    pub fn asks(&self) -> &[Ask] {
        &self.asks
    }

    /// The service with this id, where there is one on this machine.
    #[must_use]
    pub fn service(&self, id: &str) -> Option<&Filler> {
        self.services.iter().find(|one| one.id == id)
    }

    /// The upstream `adapter` stands in front of: the service of its own plugin it names
    /// in `fronts`, and nothing where it names none or its plugin brings no such service.
    #[must_use]
    pub fn fronted_by(&self, adapter: &Filler) -> Option<&Filler> {
        let upstream = adapter.fronts.as_deref()?;
        let plugin = adapter.brought_by()?;
        self.services
            .iter()
            .find(|one| one.id == upstream && one.brought_by() == Some(plugin))
    }

    /// The adapter of `upstream`'s own plugin that stands in front of it, where one does.
    #[must_use]
    pub fn fronting(&self, upstream: &Filler) -> Option<&Filler> {
        let plugin = upstream.brought_by()?;
        self.services.iter().find(|one| {
            one.fronts.as_deref() == Some(upstream.id.as_str()) && one.brought_by() == Some(plugin)
        })
    }

    /// Every service on this machine, the stack's first, each in the order it is declared.
    pub fn services(&self) -> impl Iterator<Item = &Filler> {
        self.services.iter()
    }

    /// The one service filling `capability`, with the service asking for it where one
    /// does: the one the ask settled on, or where nothing asks, the one service here
    /// providing it. Nothing where the ask is contested, or where nothing asks and other
    /// than exactly one service provides it.
    #[must_use]
    pub fn filling(&self, capability: &str) -> Option<(&Filler, Option<&Filler>)> {
        if let Some(ask) = self.asks.iter().find(|ask| ask.capability == capability) {
            let [filler] = ask.fillers.as_slice() else {
                return None;
            };
            return Some((filler, self.service(&ask.by)));
        }
        let mut providing = self
            .services
            .iter()
            .filter(|one| one.provides.iter().any(|provided| provided == capability));
        let one = providing.next()?;
        providing.next().is_none().then_some((one, None))
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
        if !matches!(filler.origin, Origin::Plugin { .. }) || filler.stands_in() {
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

impl Fillers {
    /// Every setting a credential of one installed plugin's services is kept under.
    ///
    /// The ones [`Self::setting`] answers for and nothing else, so a name another service
    /// here takes is never among them: what is taken away with a plugin is what was kept
    /// for it.
    #[must_use]
    pub fn kept_for(&self, plugin: &str) -> Vec<String> {
        self.services
            .iter()
            .filter(|one| matches!(&one.origin, Origin::Plugin { named } if named == plugin))
            .flat_map(|one| {
                crate::config::CREDENTIAL_SUFFIXES
                    .iter()
                    .filter_map(move |suffix| self.setting(one, suffix))
            })
            .collect()
    }
}

/// Whether a credential `owner` holds may be handed to `recipient`: the one gate every
/// credential crosses between two services here.
///
/// **To the stack's own, to the owner's own plugin, and to a first-party plugin from
/// the stack or another first-party plugin.** Any other plugin's service is a stranger's
/// code: what it is handed it can keep, and nothing could take it back. A plugin's
/// credential reaching the stack's own service is a plugin standing in for a bundled one
/// being wired, which is what it was installed for.
#[must_use]
pub fn crosses(owner: Holder<'_>, recipient: Holder<'_>) -> bool {
    match (owner, recipient) {
        (_, Holder::Stack) | (Holder::Stack | Holder::FirstParty(_), Holder::FirstParty(_)) => true,
        (
            Holder::FirstParty(owner) | Holder::ThirdParty(owner),
            Holder::FirstParty(recipient) | Holder::ThirdParty(recipient),
        ) => owner == recipient,
        _ => false,
    }
}

impl Fillers {
    /// Whether `owner`'s credential may be handed to `recipient`: where [`crosses`] lets
    /// it, or where the operator granted it to `recipient`'s plugin by the setting it is
    /// held under (`F14-R20`).
    #[must_use]
    pub fn hands(&self, owner: &Filler, recipient: &Filler) -> bool {
        crosses(owner.holder(), recipient.holder())
            || self
                .setting(owner, crate::config::API_KEY_SUFFIX)
                .is_some_and(|setting| recipient.granted.contains(&setting))
    }
}

/// Who holds a credential, as whether it may cross to another is judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holder<'a> {
    /// The stack's own services.
    Stack,
    /// A plugin this build embeds as first-party.
    FirstParty(&'a str),
    /// Any other plugin.
    ThirdParty(&'a str),
    /// Nothing that can be named as holding it.
    Nobody,
}

/// The name a credential `filler` holds would be kept under, ending in `holds`, before
/// anything is refused: the stack's own name for a service a first-party plugin stands
/// in for.
fn spelled(filler: &Filler, holds: &str) -> String {
    match &filler.origin {
        Origin::Plugin { named } if !filler.stands_in() => {
            crate::config::for_plugin(named, &filler.id, holds)
        }
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
        confined_to: project
            .map(|project| crate::app::targets::service_config_dir(project, &service.id)),
        media_types: service.media_types.clone(),
        provides: service.provides.clone(),
        contracts: Vec::new(),
        first_party: false,
        granted: Vec::new(),
        majors: service.majors(),
        fronts: None,
        native: None,
        replaces: None,
        via: service.network.as_ref().map(|network| network.via.clone()),
    }
}

/// One installed plugin's service, as something reaching it needs it.
///
/// Reached at its own id until [`Fillers::trusting`] places one taking `network.via` at
/// the service whose network it runs inside.
fn brought(plugin: &str, placed: &Placed, project: Option<&Path>, first_party: bool) -> Filler {
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
        media_types: placed.media_types.clone(),
        provides: placed.provides.clone(),
        contracts: placed.speaks.clone(),
        first_party,
        granted: Vec::new(),
        majors: lemonfiber_manifest::majors(&placed.tag),
        fronts: placed.fronts.clone(),
        native: placed.native.clone(),
        replaces: placed.replaces.clone(),
        via: placed
            .fields
            .network
            .as_ref()
            .map(|network| network.via.clone()),
    }
}

/// The name the stack's network resolves a bundled service to: its own id, or the id of
/// the service whose network it runs inside.
fn host(service: &Service, services: &[Service]) -> String {
    service
        .inside(services)
        .map_or_else(|| service.id.clone(), |host| host.id.clone())
}

#[cfg(test)]
impl Fillers {
    /// The services `services` alone, asking nothing and linking nothing by name.
    pub(crate) fn holding(services: Vec<Filler>) -> Self {
        Self {
            services,
            asks: Vec::new(),
            named: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests;
