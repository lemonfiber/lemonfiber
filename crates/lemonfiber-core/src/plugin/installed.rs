//! What survives an install, so nothing later has to read the manifest again.
//!
//! Not [`crate::self_update::installed`], which answers how this binary arrived on
//! the machine. This is the record of what installing somebody else's plugin
//! decided, and it is the only memory of it: the manifest is the author's file and
//! may be edited, moved or deleted the moment the install is done, and a run that
//! re-read it would be answering a question about a document rather than about the
//! machine.
//!
//! **It holds what was decided, never what can be worked out.** The compose profile
//! is the plugin's id with a word in front of it; the address behind a published port
//! is the tier rendered by lemonfiber's own rule; the source of the configuration
//! mount is lemonfiber's own directory named for the service. None of those is here,
//! because a copy of a derivation is free to disagree with the derivation. What is
//! here is the set of facts nothing can recover: which image, pinned to which digest,
//! on which tier, with its own directory mounted where.
//!
//! **Two defaults are resolved on the way in, and one is not.** Where a manifest
//! names no configuration directory the published fallback is written down, because
//! the record says where the directory *is* and a fallback that later moved would
//! make the record disagree with the container. The same goes for the label a service
//! answers on. Which dashboard group a tier belongs to is the stack's answer rather
//! than the format's, so nothing declared is kept as nothing declared.

use lemonfiber_plugin::{Manifest, Service};
use serde::{Deserialize, Serialize};

mod reached;

pub use reached::Reached;

/// One service of an installed plugin, as it was placed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginPlaced")]
pub struct Placed {
    /// The service's id, which is the name its container is written under.
    pub service: String,
    /// The registry path, carrying no pin of its own.
    pub image: String,
    /// The digest that fixes what runs.
    pub digest: String,
    /// The readable name that digest went by when it was installed.
    pub tag: String,
    /// Where inside the container its one configuration directory is mounted.
    ///
    /// Resolved rather than optional. The record answers where the directory is, and
    /// a run that re-derived the fallback would answer for a container it did not
    /// write the day that fallback moved.
    pub config_path: String,
    /// Whether the library is mounted for it.
    pub takes_data: bool,
    /// How it is reached, or nothing where it has no listener.
    pub reached: Option<Reached>,
    /// Every core capability this one service fills, which is what makes it a candidate
    /// when the stack asks for one.
    ///
    /// Per service rather than read off the plugin's whole list, because a wiring
    /// reaches a service and not a plugin: of a plugin's two services, the one that
    /// fills a capability is the one an ask for it would reach. Defaulted for a record
    /// written before this was kept, which reads as filling nothing — a service nothing
    /// is wired to, rather than one wired to on a guess.
    #[serde(default)]
    pub provides: Vec<String>,
    /// What it is called, for a reader, which is what its dashboard entry is listed as;
    /// a record written before this was kept lists it by its id.
    #[serde(default)]
    pub name: String,
    /// What the plugin says it does for the operator, which is what its dashboard entry
    /// says beside it.
    #[serde(default)]
    pub description: String,
    /// The adapter lemonfiber reaches it through, where the plugin named one.
    ///
    /// Defaulted for a record written before this was kept, which reads as naming
    /// none: a service operated generically, as it was when installed.
    #[serde(default)]
    pub api: Option<lemonfiber_manifest::Api>,
    /// The port it answers on inside the stack's network, where it declared one.
    #[serde(default)]
    pub listens: Option<u16>,
    /// The media it files, in the stack manifest's vocabulary, which decides what it
    /// comes to in each service that asks for what it provides; none in an older record.
    #[serde(default)]
    pub media_types: Vec<String>,
    /// The stack's own networks it joins beside the default one, because a stack service
    /// it stands in for is on them.
    ///
    /// Settled at install from the stack it was installed beside and written down, so
    /// the container lemonfiber writes for it stays a function of this record alone.
    /// Defaulted for a record written before this was kept, which reads as joining none
    /// and staying on the default network.
    #[serde(default)]
    pub networks: Vec<String>,
    /// Each capability contract it answers as an adapter, as `capability@major`.
    #[serde(default)]
    pub speaks: Vec<String>,
    /// The service of the same plugin it stands in front of, as an adapter.
    #[serde(default)]
    pub fronts: Option<String>,
    /// The privileged shape lemonfiber writes for it, where it took one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<lemonfiber_plugin::Shape>,
    /// Every capability it asks for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asks: Vec<Asking>,
}

/// One capability a placed service asks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginAsking")]
pub struct Asking {
    /// The core capability asked for.
    pub capability: String,
    /// Whether every service that fills it is reached rather than one.
    #[serde(default)]
    pub each: bool,
}

impl Placed {
    /// One service, as installing it settles it.
    fn of(manifest: &Manifest, service: &Service) -> Self {
        Self {
            service: service.id.clone(),
            image: service.image.clone(),
            digest: service.digest.clone(),
            tag: service.tag.clone(),
            config_path: service.configuration().to_owned(),
            takes_data: service.takes_data,
            reached: Reached::of(service, manifest.entry(service)),
            provides: service
                .provides
                .iter()
                .filter(|name| lemonfiber_plugin::vocabulary::is_core_name(name))
                .cloned()
                .collect(),
            name: service.name.clone(),
            description: manifest.plugin.description.clone(),
            api: service.api.clone(),
            listens: service.listens,
            media_types: service.media_types.clone(),
            networks: Vec::new(),
            speaks: service.speaks.clone(),
            fronts: service.fronts.clone(),
            shape: service.shape,
            asks: manifest
                .asking
                .iter()
                .filter(|ask| {
                    manifest
                        .asks(ask.service.as_deref())
                        .is_some_and(|asker| asker.id == service.id)
                })
                .map(|ask| Asking {
                    capability: ask.capability.clone(),
                    each: ask.each,
                })
                .collect(),
        }
    }

    /// What it is called in front of an operator: the name it was installed with, or its
    /// id where the record was written before names were kept.
    #[must_use]
    pub fn called(&self) -> &str {
        if self.name.is_empty() {
            &self.service
        } else {
            &self.name
        }
    }

    /// The port this machine reaches the service on, where it publishes one.
    ///
    /// Nothing where it publishes none, which is a service with no listener rather
    /// than one on an address nobody wrote down — so a proof against it has nowhere
    /// to ask rather than somewhere to guess.
    #[must_use]
    pub fn published(&self) -> Option<u16> {
        self.reached.as_ref().map(Reached::port)
    }
}

/// A service of `would` named, once spelled as an environment name, as a service of
/// another installed plugin already is: the one it would bring, the one already there,
/// and whose that is.
///
/// lemonfiber writes a plugin's container under its service's id and keeps what the
/// service holds in settings named after it, so two such services would be one container
/// and one credential. The same plugin's own record is not another plugin's, so an update
/// replacing a version is not held to the names of the version it replaces.
#[must_use]
pub fn spelled_alike(
    would: &Installed,
    installed: &[Installed],
) -> Option<(String, String, String)> {
    for other in installed
        .iter()
        .filter(|other| other.plugin != would.plugin)
    {
        for theirs in &other.services {
            let spelled = lemonfiber_manifest::environment_name(&theirs.service);
            let ours = would
                .services
                .iter()
                .find(|ours| lemonfiber_manifest::environment_name(&ours.service) == spelled);
            if let Some(ours) = ours {
                return Some((
                    ours.service.clone(),
                    theirs.service.clone(),
                    other.plugin.clone(),
                ));
            }
        }
    }
    None
}

/// One plugin's install, as it was decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginInstalled")]
pub struct Installed {
    /// The plugin's id: the name it is installed and journalled under.
    pub plugin: String,
    /// The plugin's own content version, as it stood when it was installed.
    pub version: String,
    /// What the plugin calls itself, for a person to read.
    ///
    /// Absent on a record written before it was kept, and never filled in from the id:
    /// a name is the author's, and one made up here would be lemonfiber's.
    #[serde(default)]
    pub name: Option<String>,
    /// What it does for the operator, in its author's words. Absent alike.
    #[serde(default)]
    pub description: Option<String>,
    /// What was placed, one entry per service the plugin declares.
    pub services: Vec<Placed>,
    /// Every core capability its services fill, as the install settled them.
    ///
    /// Core names only. A capability of the plugin's own is namespaced, nothing asks
    /// for it, and it is inert by design — so a removal that named one as about to go
    /// unfilled would be warning about something nothing was reaching for.
    ///
    /// Kept for the question a removal has to answer before it happens: what this
    /// machine would have nothing filling once the plugin is off it. Asking the
    /// manifest would be asking a file that may be gone.
    ///
    /// Defaulted for a register written before the field existed, which reads as a
    /// plugin that fills nothing — the answer that names no capability rather than the
    /// one that invents one.
    #[serde(default)]
    pub provides: Vec<String>,
    /// Every row it adds to a register lemonfiber already runs, as the install
    /// settled them.
    ///
    /// Kept here rather than read back off the plugin's own files, for the reason
    /// everything else on this record is: the author's directory may be gone the
    /// moment an install is done, and a doctor run a month later is a question about
    /// this machine rather than about a document. This record is the one answer, so a
    /// row that runs is a row that was declared at install and has not changed under
    /// anybody since.
    ///
    /// Defaulted for a register written before this field existed, which reads as a
    /// plugin that contributes nothing — the same answer a plugin that contributes
    /// nothing gets, and the only one that can be given about a record that does not
    /// say.
    #[serde(default)]
    pub contributions: Vec<lemonfiber_plugin::Contribution>,
    /// What the plugin declared about itself: where it is published, whether it was
    /// reviewed, what it claims, what it may change, where it may reach and what it
    /// will hold.
    #[serde(default)]
    pub declared: super::declared::Declaration,
    /// Every recipe it declares: each call in order with the adapter it reaches through,
    /// and every value that could leave for somewhere else.
    ///
    /// Empty for a plugin that declares none and for a record written before these were
    /// kept, so a list is always there to read.
    #[serde(default)]
    pub recipes: Vec<super::recipes::Recipe>,
    /// Every adapter of lemonfiber's its own services name, each said to be lemonfiber's.
    #[serde(default)]
    pub adapters: Vec<super::recipes::Named>,
    /// The source it was installed from, as the operator named it.
    ///
    /// Empty for a record written before this was kept. A rehearsal's account carries
    /// the source it was asked about, because that is what it would record.
    #[serde(default)]
    pub from: String,
    /// The commit it was installed at, where it came from a git source.
    ///
    /// The one commit the revision named at install resolved to, so what was installed
    /// can be told from whatever that source serves now. Empty for a plugin installed
    /// from a directory, which has no revision to name, and for a record written before
    /// this was kept.
    #[serde(default)]
    pub revision: String,
    /// What signed it: the key the catalogue index it was resolved through verified
    /// against, named with its fingerprint; empty where nothing signed it.
    #[serde(default)]
    pub signed: String,
    /// The SHA-256 of the manifest it was installed from, in lower-case hexadecimal.
    #[serde(default)]
    pub manifest: String,
    /// When it was installed, as the record stamps every change: whole seconds since
    /// the epoch.
    ///
    /// The install's own stamp, the one its changes are journalled under, so the
    /// listing and the history name the same moment. Empty where the record predates
    /// it; a rehearsal's account carries the moment it was asked, which is the stamp
    /// the install would have run under.
    #[serde(default)]
    pub installed_at: String,
}

impl Installed {
    /// What installing this manifest would settle.
    ///
    /// A function of the manifest alone, so every decision it takes can be put in
    /// front of a test without a directory, a container engine or a stack.
    ///
    /// **Which service each contributed row asks is settled here and written down.**
    /// The rule for it is the manifest's — a row that names none asks the plugin's
    /// one service, and a plugin with several leaves it unsettled — and the manifest
    /// is the only place that rule can be applied, because it is the only place both
    /// the row and the service list exist. Applying it again later against this
    /// record would be a second answer to a question already answered.
    #[must_use]
    pub fn of(manifest: &Manifest) -> Self {
        Self {
            plugin: manifest.plugin.id.clone(),
            version: manifest.plugin.version.clone(),
            name: Some(manifest.plugin.name.clone()),
            description: Some(manifest.plugin.description.clone()),
            services: manifest
                .services
                .iter()
                .map(|service| Placed::of(manifest, service))
                .collect(),
            provides: filled(manifest),
            contributions: manifest
                .contributions
                .iter()
                .map(|entry| lemonfiber_plugin::Contribution {
                    service: manifest
                        .asks(entry.service.as_deref())
                        .map(|service| service.id.clone()),
                    ..entry.clone()
                })
                .collect(),
            declared: super::declared::Declaration::of(manifest),
            recipes: super::recipes::declared(manifest),
            adapters: super::recipes::named(manifest),
            from: String::new(),
            revision: String::new(),
            signed: String::new(),
            manifest: String::new(),
            installed_at: String::new(),
        }
    }

    /// The same record, as installed from this source at this moment.
    ///
    /// Apart from [`Self::of`], which is a function of the manifest alone and stays
    /// one: where it came from and when are facts about this machine, known only to the
    /// run that installs it.
    #[must_use]
    pub fn installed(self, from: &std::path::Path, at: &str) -> Self {
        Self {
            from: from.display().to_string(),
            installed_at: at.to_owned(),
            ..self
        }
    }

    /// The same record, with the networks each service joins beside the stack it is
    /// installed beside.
    ///
    /// Apart from [`Self::of`] for the reason [`Self::installed`] is: the networks are a
    /// fact about the stack on this machine, not about the manifest.
    #[must_use]
    pub fn joining(self, joins: &super::joining::Joins) -> Self {
        Self {
            services: self
                .services
                .into_iter()
                .map(|placed| Placed {
                    networks: joins.of_service(&self.plugin, &placed),
                    ..placed
                })
                .collect(),
            ..self
        }
    }

    /// The same record, with the adapter of every recipe call to one of the stack's own
    /// services.
    #[must_use]
    pub fn reaching(self, stack: &lemonfiber_manifest::Manifest) -> Self {
        Self {
            recipes: super::recipes::reaching(self.recipes, stack),
            ..self
        }
    }

    /// The same record, as installed from a git source at one commit.
    #[must_use]
    pub fn fetched(self, from: &str, revision: &str) -> Self {
        Self {
            from: from.to_owned(),
            revision: revision.to_owned(),
            ..self
        }
    }

    /// The same record, as resolved through a catalogue index this key signed: reviewed,
    /// and carrying what signed it.
    #[must_use]
    pub fn vouched(self, signed: &str) -> Self {
        Self {
            signed: signed.to_owned(),
            declared: super::declared::Declaration {
                reviewed: true,
                ..self.declared
            },
            ..self
        }
    }
}

/// Every core capability a manifest's services fill, in declaration order and once
/// each.
///
/// Read off `provides` rather than off the claim blocks, because `provides` is what the
/// wiring asks against — a claim is the evidence for one and the two are held together
/// when the manifest is read. Namespaced names are left out: nothing asks for one, so
/// nothing can be left without it.
fn filled(manifest: &Manifest) -> Vec<String> {
    let mut named: Vec<String> = Vec::new();
    for capability in manifest
        .services
        .iter()
        .flat_map(|service| service.provides.iter())
        .filter(|name| lemonfiber_plugin::vocabulary::is_core_name(name))
    {
        if !named.iter().any(|held| held == capability) {
            named.push(capability.clone());
        }
    }
    named
}

/// Where a service published on this machine is reached.
///
/// The loopback address rather than the label a household service also answers on: a
/// plugin's service is asked by lemonfiber, from the machine its container runs on, and
/// the published port is what is there whichever tier the service is on. The same
/// address the bundled services are proved at.
const HERE: &str = "http://127.0.0.1";

/// Where each installed service answers, by the id its manifest gave it.
///
/// One answer to *where is this reached*, for the two callers that ask. An install asks
/// it to put the plugin's own proofs; the diagnostics register asks it to put the rows
/// the plugin contributed. Two answers would be two ways of reaching the same service,
/// and the one that fell behind would be asking a port nothing is listening on.
///
/// A service that publishes no port is absent rather than present with a guessed
/// address, which is what lets a caller say *there is nowhere to ask it* instead of
/// asking somewhere and reporting what that answered.
#[must_use]
pub fn answering(installed: &[Installed]) -> std::collections::BTreeMap<String, String> {
    installed
        .iter()
        .flat_map(|one| one.services.iter())
        .filter_map(|placed| {
            placed
                .published()
                .map(|port| (placed.service.clone(), format!("{HERE}:{port}")))
        })
        .collect()
}

#[cfg(test)]
mod tests;
