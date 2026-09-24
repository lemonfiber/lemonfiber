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
use thiserror::Error;

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

/// Why a register could not be read.
///
/// Two ways, and neither of them is *there is no file*. No file, an empty file and a
/// machine that has never installed anything are one answer — an empty register — and
/// that answer is not a fault. These two are: a file that is there and will not parse,
/// and one that parses and says two different things about the same plugin.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Unreadable {
    /// The record is there and this build cannot read it.
    #[error("the record of installed plugins could not be read: {0}")]
    Damaged(String),

    /// The record names one plugin twice.
    #[error("the record of installed plugins holds {0} twice, so what is installed under that name cannot be said")]
    Twice(String),
}

/// Why a plugin could not be recorded as installed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{plugin} is already installed, at version {version}")]
pub struct Already {
    /// Which plugin the record already holds.
    pub plugin: String,
    /// The version it holds for it.
    pub version: String,
}

/// Every plugin this machine has installed.
///
/// Kept in one file beside the settings rather than one file per plugin: what an
/// operator asks is *what is installed*, a directory answers that only by being
/// listed, and a half-written directory has no shape a read can refuse.
///
/// The order is the plugins' own ids, so the file reads the same twice and a diff of
/// it says what changed rather than where something was appended.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Register {
    /// One record per installed plugin.
    #[serde(default)]
    installed: Vec<Installed>,
}

impl Register {
    /// Nothing installed.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            installed: Vec::new(),
        }
    }

    /// What the record holds, or why it cannot be said.
    ///
    /// **A damaged record is refused rather than read as empty**, which is where this
    /// parts company with every other small record beside the settings. Those hold an
    /// answer that can be given again: a forgotten preference is asked for a second
    /// time, and the cost is a question. This one is the only memory that a stranger's
    /// service is on this machine at all — reading it as empty would report a stack
    /// with a plugin in it as a stack with none, and every later reading of what is
    /// installed, what is overridden and what a removal would put back would be
    /// confidently wrong.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] where the text is not a register this build can read, or names
    /// one plugin twice.
    pub fn parse(text: &str) -> Result<Self, Unreadable> {
        if text.trim().is_empty() {
            return Ok(Self::empty());
        }
        let read: Self =
            serde_json::from_str(text).map_err(|why| Unreadable::Damaged(why.to_string()))?;
        read.once_each()?;
        Ok(read)
    }

    /// Whether any plugin appears twice.
    ///
    /// Checked on the way in rather than trusted to the writer. The id is the name a
    /// plugin is installed and journalled under, so two records for one name is two
    /// answers to *what is installed as this* — and a reader taking the first would
    /// silently prefer whichever was written earlier.
    fn once_each(&self) -> Result<(), Unreadable> {
        let mut seen: Vec<&str> = Vec::new();
        for one in &self.installed {
            if seen.contains(&one.plugin.as_str()) {
                return Err(Unreadable::Twice(one.plugin.clone()));
            }
            seen.push(&one.plugin);
        }
        Ok(())
    }

    /// As it is kept: sorted by plugin id, one trailing newline.
    ///
    /// `None` only where it will not serialise, which strings and numbers cannot.
    #[must_use]
    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string_pretty(self)
            .ok()
            .map(|text| text + "\n")
    }

    /// What is installed, in the order the record keeps them.
    #[must_use]
    pub fn installed(&self) -> &[Installed] {
        &self.installed
    }

    /// What is recorded for this plugin, where anything is.
    #[must_use]
    pub fn holds(&self, plugin: &str) -> Option<&Installed> {
        self.installed.iter().find(|one| one.plugin == plugin)
    }

    /// Record an install, keeping the order the file is read back in.
    ///
    /// # Errors
    ///
    /// [`Already`] where this plugin is recorded. An install over an install is an
    /// update, which reverses one set of changes and applies another; treating it as
    /// a write would leave the record describing the new version and the machine
    /// carrying both.
    pub fn record(&mut self, one: Installed) -> Result<(), Already> {
        if let Some(held) = self.holds(&one.plugin) {
            return Err(Already {
                plugin: held.plugin.clone(),
                version: held.version.clone(),
            });
        }
        let at = self
            .installed
            .partition_point(|held| held.plugin < one.plugin);
        self.installed.insert(at, one);
        Ok(())
    }

    /// Take a plugin out of the record.
    ///
    /// Silent about a name it does not hold, because the caller has already refused
    /// that case by name and a second refusal here would be a second answer to a
    /// question already settled. What this promises is the state afterwards: whatever
    /// it held about that plugin, it holds nothing now.
    pub fn forget(&mut self, plugin: &str) {
        self.installed.retain(|one| one.plugin != plugin);
    }
}

#[cfg(test)]
mod tests {
    use lemonfiber_plugin::{Bind, Manifest};

    use super::super::reports::{Install, Installs};
    use super::{Installed, Placed, Reached, Register, Unreadable};

    /// A plugin declaring two services, though the reader admits one at a time.
    ///
    /// Two because what is under test is the record: with one service every
    /// per-service decision is indistinguishable from a per-plugin one, and the two
    /// here differ in exactly the ways the format exists to record — one faces the
    /// household and the other is an operator surface, one keeps its state where the
    /// convention says and the other says nothing at all.
    const MANIFEST: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.2.0"
description = "Reads your comics on any browser"
without_it  = "Files on disk, no way to read them"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "example.invalid/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
port        = 25600
bind        = "lan"
criticality = "important"
takes_data  = true
config_path = "/config"

[[service]]
id          = "komga-sidecar"
name        = "Komga's indexer"
image       = "example.invalid/komga-sidecar"
digest      = "sha256:0000cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "0.4.1"
port        = 9000
bind        = "loopback"
criticality = "enhancing"

[[wiring]]
hostname        = "comics"
dashboard_group = "Library"
"#;

    /// What installing the fixture settles, with whatever departure the case under
    /// test needs made to the manifest first.
    ///
    /// Nothing where the fixture stopped being a manifest, so a fixture that broke
    /// fails the assertion it was written for rather than somewhere further down.
    fn installed(change: impl FnOnce(&mut Manifest)) -> Option<Installed> {
        let mut manifest = Manifest::from_toml(MANIFEST).ok()?;
        change(&mut manifest);
        Some(Installed::of(&manifest))
    }

    /// **Every core capability its services fill, once each, and nothing namespaced.**
    /// A capability of the plugin's own is inert by design — nothing asks for one, so
    /// nothing can be left without it, and a removal naming one would be warning about
    /// something nobody was reaching for.
    #[test]
    fn what_a_plugin_fills_is_the_core_names_its_services_provide() {
        let record = installed(|manifest| {
            for service in &mut manifest.services {
                service.provides = vec![
                    "media.serve".to_owned(),
                    "komga:kobo-sync".to_owned(),
                    "media.serve".to_owned(),
                ];
            }
        });

        assert_eq!(
            record.as_ref().map(|one| one.provides.clone()),
            Some(vec!["media.serve".to_owned()]),
            "the core one, once, and not the plugin's own"
        );
        assert_eq!(
            whole().map(|one| one.provides),
            Some(Vec::new()),
            "and a plugin whose services fill nothing fills nothing"
        );
    }

    /// A change to the first service the fixture declares.
    ///
    /// Reached rather than indexed: a fixture that stopped declaring a service would
    /// otherwise end the run rather than fail the assertion it was written for.
    fn set(manifest: &mut Manifest, change: impl FnOnce(&mut lemonfiber_plugin::Service)) {
        if let Some(service) = manifest.services.first_mut() {
            change(service);
        }
    }

    /// The fixture as its author wrote it.
    fn whole() -> Option<Installed> {
        installed(|_| ())
    }

    /// How one of a record's services was placed.
    fn placed(record: Option<&Installed>, service: &str) -> Option<Placed> {
        record?
            .services
            .iter()
            .find(|one| one.service == service)
            .cloned()
    }

    /// Where one of them keeps its own state.
    fn config_path(record: Option<&Installed>, service: &str) -> Option<String> {
        placed(record, service).map(|one| one.config_path)
    }

    #[test]
    fn the_configuration_directory_a_service_declares_survives_the_install() {
        assert_eq!(
            config_path(whole().as_ref(), "komga"),
            Some("/config".to_owned())
        );
    }

    /// The fallback is written down rather than left to be worked out again, which is
    /// what makes the record answer where the directory *is*.
    #[test]
    fn a_service_that_declares_no_configuration_directory_records_the_published_one() {
        assert_eq!(
            config_path(whole().as_ref(), "komga-sidecar"),
            Some(lemonfiber_plugin::CONFIGURATION.to_owned())
        );
    }

    /// The manifest's own declaration, not a convention the record repeated. An
    /// image keeping its state elsewhere is the whole reason the field exists.
    #[test]
    fn a_service_whose_image_reads_its_state_elsewhere_records_that_path() {
        let record = installed(|manifest| {
            set(manifest, |service| {
                service.config_path = Some("/app/data".to_owned());
            });
        });
        assert_eq!(
            config_path(record.as_ref(), "komga"),
            Some("/app/data".to_owned())
        );
    }

    #[test]
    fn what_runs_is_recorded_as_the_path_and_the_digest_that_pins_it() {
        let one = placed(whole().as_ref(), "komga");
        assert_eq!(
            one.as_ref().map(|one| one.image.clone()),
            Some("example.invalid/komga".to_owned())
        );
        assert_eq!(
            one.as_ref().map(|one| one.digest.starts_with("sha256:")),
            Some(true)
        );
        assert_eq!(one.map(|one| one.tag), Some("1.11.0".to_owned()));
    }

    /// The tier is each service's own, read from the field that decides it rather
    /// than from anything held beside the plugin.
    #[test]
    fn each_service_keeps_the_tier_its_own_declaration_names() {
        let record = whole();
        assert_eq!(
            placed(record.as_ref(), "komga").map(|one| one.reached),
            Some(Some(Reached::Household {
                port: 25600,
                hostname: "comics".to_owned(),
                group: Some("Library".to_owned()),
            }))
        );
        assert_eq!(
            placed(record.as_ref(), "komga-sidecar").map(|one| one.reached),
            Some(Some(Reached::Loopback {
                port: 9000,
                group: Some("Library".to_owned()),
            }))
        );
    }

    /// The correction that mattered: only the proxy is the wider tier's alone. An
    /// operator surface still appears on the bundled dashboard, so the group and the
    /// tier its link is rendered from both have to survive the install.
    #[test]
    fn a_loopback_service_keeps_the_dashboard_group_and_the_tier_its_link_is_built_from() {
        let one = placed(whole().as_ref(), "komga-sidecar").and_then(|one| one.reached);
        assert_eq!(one.as_ref().map(Reached::port), Some(9000));
        assert_eq!(one.as_ref().map(Reached::group), Some(Some("Library")));
        assert_eq!(one.as_ref().map(Reached::hostname), Some(None));
    }

    /// The three readings are the facts the arms carry, so a caller need not write
    /// the match itself and get one arm of it wrong.
    #[test]
    fn what_a_household_service_is_reached_by_reads_the_same_either_way() {
        let one = placed(whole().as_ref(), "komga").and_then(|one| one.reached);
        assert_eq!(one.as_ref().map(Reached::port), Some(25600));
        assert_eq!(one.as_ref().map(Reached::hostname), Some(Some("comics")));
        assert_eq!(one.as_ref().map(Reached::group), Some(Some("Library")));
    }

    /// A label is a fact about one container. Defaulting to the plugin's id would
    /// give two services of one plugin the same address.
    #[test]
    fn a_service_the_manifest_names_no_label_for_answers_on_its_own_id() {
        let record = installed(|manifest| manifest.wirings.clear());
        assert_eq!(
            placed(record.as_ref(), "komga").map(|one| one.reached),
            Some(Some(Reached::Household {
                port: 25600,
                hostname: "komga".to_owned(),
                group: None,
            }))
        );
    }

    /// The tier decides, and the record cannot say otherwise: a loopback service has
    /// nowhere to put a hostname, whatever the manifest asked for.
    #[test]
    fn a_loopback_service_carries_no_hostname_even_where_one_is_declared() {
        let record =
            installed(|manifest| set(manifest, |service| service.bind = Some(Bind::Loopback)));
        assert_eq!(
            placed(record.as_ref(), "komga").map(|one| one.reached),
            Some(Some(Reached::Loopback {
                port: 25600,
                group: Some("Library".to_owned()),
            }))
        );
    }

    #[test]
    fn a_service_with_no_listener_is_recorded_as_reached_by_nothing() {
        let record = installed(|manifest| {
            set(manifest, |service| {
                service.port = None;
                service.bind = None;
            });
        });
        assert_eq!(
            placed(record.as_ref(), "komga").map(|one| one.reached),
            Some(None)
        );
    }

    /// A port with no tier is a manifest the reader refuses. Were one to reach here
    /// the record says it is reached by nothing rather than choosing a tier for it —
    /// the one answer that cannot put an admin surface on the household network.
    #[test]
    fn a_port_with_no_tier_is_recorded_as_reached_by_nothing_rather_than_placed() {
        let record = installed(|manifest| set(manifest, |service| service.bind = None));
        assert_eq!(
            placed(record.as_ref(), "komga").map(|one| one.reached),
            Some(None)
        );
    }

    #[test]
    fn whether_the_library_is_mounted_is_each_services_own_answer() {
        let record = whole();
        assert_eq!(
            placed(record.as_ref(), "komga").map(|one| one.takes_data),
            Some(true)
        );
        assert_eq!(
            placed(record.as_ref(), "komga-sidecar").map(|one| one.takes_data),
            Some(false)
        );
    }

    #[test]
    fn the_plugin_is_recorded_under_its_id_and_the_version_that_was_installed() {
        let record = whole();
        assert_eq!(
            record.as_ref().map(|one| (
                one.plugin.clone(),
                one.version.clone(),
                one.services.len()
            )),
            Some(("komga".to_owned(), "1.2.0".to_owned(), 2))
        );
    }

    /// A register holding whatever the fixture settles, for the cases below.
    fn register(plugins: &[&str]) -> Register {
        let mut register = Register::empty();
        for id in plugins {
            let one = installed(|manifest| manifest.plugin.id = (*id).to_owned());
            assert_eq!(one.map(|one| register.record(one)), Some(Ok(())));
        }
        register
    }

    #[test]
    fn a_record_written_is_a_record_read_back() {
        let register = register(&["komga"]);
        let text = register.to_json().unwrap_or_default();
        assert_eq!(Register::parse(&text), Ok(register));
    }

    #[test]
    fn a_machine_with_no_record_has_nothing_installed() {
        assert_eq!(Register::parse(""), Ok(Register::empty()));
        assert_eq!(Register::parse("   \n"), Ok(Register::empty()));
        assert!(Register::empty().installed().is_empty());
        assert_eq!(Register::empty().holds("komga"), None);
    }

    /// The gate this record exists to pass. Every other small record beside the
    /// settings reads a damaged file as its default; this one must not, because the
    /// default is *nothing is installed* and that is a false answer about somebody
    /// else's service running on the machine.
    #[test]
    fn a_damaged_record_is_refused_rather_than_read_as_nothing_installed() {
        let refused = Register::parse("{ not json at all");
        assert!(
            matches!(refused, Err(Unreadable::Damaged(_))),
            "got: {refused:?}"
        );
        let said = refused.err().map(|why| why.to_string()).unwrap_or_default();
        assert!(said.contains("could not be read"), "got: {said}");
    }

    /// A field this build does not know is a record something else wrote, and
    /// reading the half it recognises would report a narrower install than happened.
    #[test]
    fn a_record_carrying_something_this_build_does_not_know_is_refused() {
        let refused = Register::parse(r#"{"installed": [], "running": true}"#);
        assert!(
            matches!(refused, Err(Unreadable::Damaged(_))),
            "got: {refused:?}"
        );
    }

    #[test]
    fn a_record_naming_one_plugin_twice_is_refused_naming_it() {
        let one = whole()
            .as_ref()
            .and_then(|one| serde_json::to_string(one).ok())
            .unwrap_or_default();
        let refused = Register::parse(&format!(r#"{{"installed": [{one}, {one}]}}"#));
        assert_eq!(refused, Err(Unreadable::Twice("komga".to_owned())));
        let said = refused.err().map(|why| why.to_string()).unwrap_or_default();
        assert!(said.contains("komga"), "got: {said}");
        assert!(said.contains("twice"), "got: {said}");
    }

    /// An install over an install is an update, which reverses one set of changes and
    /// applies another. Writing it as an install would leave the record describing
    /// one version and the machine carrying two.
    #[test]
    fn installing_over_an_install_is_refused_naming_what_is_there() {
        let mut register = register(&["komga"]);
        let said = whole()
            .and_then(|one| register.record(one).err())
            .map(|why| why.to_string())
            .unwrap_or_default();
        assert!(said.contains("komga"), "got: {said}");
        assert!(said.contains("1.2.0"), "got: {said}");
        assert_eq!(register.installed().len(), 1);
    }

    /// The file reads the same twice, so a diff of it says what changed rather than
    /// where somebody appended.
    #[test]
    fn the_record_keeps_its_plugins_in_one_order_whatever_order_they_arrived_in() {
        let one = register(&["plex", "komga", "uptime"]);
        assert_eq!(one, register(&["uptime", "plex", "komga"]));
        let held: Vec<&str> = one
            .installed()
            .iter()
            .map(|record| record.plugin.as_str())
            .collect();
        assert_eq!(held, vec!["komga", "plex", "uptime"]);
    }

    #[test]
    fn what_is_recorded_for_one_plugin_is_answerable_by_name() {
        let register = register(&["komga"]);
        assert_eq!(
            register.holds("komga").map(|one| one.version.as_str()),
            Some("1.2.0")
        );
        assert_eq!(register.holds("plex"), None);
    }

    /// The report carries both halves whichever way it was reached, so a surface has
    /// one shape to render rather than two that agree today.
    #[test]
    fn the_report_says_what_is_installed_and_what_this_run_did() {
        let read = Installs {
            installed: whole().into_iter().collect(),
            install: None,
            removal: None,
            update: None,
        };
        let done = Installs {
            removal: None,
            installed: whole().into_iter().collect(),
            install: whole().map(|would| Install {
                would,
                recorded: true,
                changes: Vec::new(),
                proofs: Vec::new(),
                against: None,
                verified: None,
                overrides: Vec::new(),
                reversed: None,
                contests: Vec::new(),
            }),
            update: None,
        };
        assert!(read.install.is_none());
        assert_eq!(done.install.map(|one| one.recorded), Some(true));
    }
}
