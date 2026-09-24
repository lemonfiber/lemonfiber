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
//!
//! **A plugin is several services.** One is what this generation of the format
//! admits, and the record is shaped for what the contract describes rather than for
//! what the reader currently allows — a record built around a single service would
//! have to be migrated by the change that admits the second, and a migration of a
//! record nobody can regenerate is the expensive kind.

use lemonfiber_plugin::{Bind, Manifest, Service};
use serde::{Deserialize, Serialize};

/// How an installed service is reached, where it is reached at all.
///
/// The tier is the arm, so the label a tier earns lives only in the arm entitled to
/// one. Only a household service is proxied — the bundled policy is that an admin
/// surface does not get a name on the household network — and a record able to carry
/// a loopback service with a hostname would be a record able to describe the thing
/// that policy exists to prevent.
///
/// **The group is on both arms, and that is not an oversight.** Only the proxy is
/// the household tier's alone; the bundled dashboard carries an entry for an
/// operator surface too, with the address it links to rendered from the tier — nine
/// of the shipped stack's own entries point at this machine. A record that kept the
/// group for the wider tier alone would leave a loopback service off the panel its
/// bundled neighbours are on.
///
/// A tier and never an address, either way: lemonfiber renders one from the other
/// exactly as it does for a bundled service, so the two-tier policy stays a property
/// of the system rather than a request a plugin made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "tier", rename_all = "lowercase", deny_unknown_fields)]
#[schemars(rename = "PluginReached")]
pub enum Reached {
    /// From this machine and nowhere else. No route, and no label to route to.
    Loopback {
        /// The port the service listens on.
        port: u16,
        /// The group on the bundled dashboard, where the manifest named one.
        #[serde(default)]
        group: Option<String>,
    },
    /// From the household, through the stack's own proxy, at this label.
    Household {
        /// The port the service listens on.
        port: u16,
        /// The single label in front of the operator's domain.
        hostname: String,
        /// The group on the bundled dashboard, where the manifest named one.
        #[serde(default)]
        group: Option<String>,
    },
}

impl Reached {
    /// How this service is reached, from what its own declaration says.
    ///
    /// Nothing where it publishes no port: a service with no listener is not reached
    /// at all, which is a third answer rather than a tier nobody chose. A port with
    /// no tier cannot be installed — the reader refuses that manifest — so it lands
    /// here as unreachable rather than being given a tier this code picked.
    fn of(service: &Service, entry: lemonfiber_plugin::Entry<'_>) -> Option<Self> {
        let group = entry.group.map(str::to_owned);
        match (service.port, service.bind) {
            (Some(port), Some(Bind::Lan)) => Some(Self::Household {
                port,
                hostname: entry.hostname.to_owned(),
                group,
            }),
            (Some(port), Some(Bind::Loopback)) => Some(Self::Loopback { port, group }),
            (Some(_), None) | (None, _) => None,
        }
    }

    /// The port the service listens on, whichever tier it is on.
    ///
    /// Read rather than matched at every call site: the port is the same fact on
    /// both arms, and a caller writing the match itself is a caller free to get one
    /// of the two wrong.
    #[must_use]
    pub const fn port(&self) -> u16 {
        match self {
            Self::Loopback { port, .. } | Self::Household { port, .. } => *port,
        }
    }

    /// The group on the bundled dashboard, where the manifest named one.
    #[must_use]
    pub fn group(&self) -> Option<&str> {
        match self {
            Self::Loopback { group, .. } | Self::Household { group, .. } => group.as_deref(),
        }
    }

    /// The label this service answers on, or nothing where its tier gives it none.
    #[must_use]
    pub fn hostname(&self) -> Option<&str> {
        match self {
            Self::Loopback { .. } => None,
            Self::Household { hostname, .. } => Some(hostname),
        }
    }
}

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
    /// What it is called, for a reader, which is what its dashboard entry is listed as.
    ///
    /// Defaulted for a record written before this was kept, which lists it by its id
    /// rather than leaving it off the panel.
    #[serde(default)]
    pub name: String,
    /// What the plugin says it does for the operator, which is what its dashboard entry
    /// says beside it.
    #[serde(default)]
    pub description: String,
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

/// One plugin's install, as it was decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginInstalled")]
pub struct Installed {
    /// The plugin's id: the name it is installed and journalled under.
    pub plugin: String,
    /// The plugin's own content version, as it stood when it was installed.
    pub version: String,
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
    /// The source it was installed from, as the operator named it.
    ///
    /// Empty for a record written before this was kept. A rehearsal's account carries
    /// the source it was asked about, because that is what it would record.
    #[serde(default)]
    pub from: String,
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
            from: String::new(),
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
