//! What the plugin catalogue offers to install, as the operator browses it.

use serde::Serialize;

use crate::instant::Instant;

/// What the catalogue offers, or why it was not read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct PluginCatalogueReport {
    /// Whether the catalogue was read, switched off, or could not be read.
    pub state: PluginCatalogueState,
    /// When the index listed was published. Present where it was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published: Option<Instant>,
    /// Which release of the catalogue the index listed is. Present where it was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<u64>,
    /// Every plugin the verified index offers, by id; empty where it was not read.
    pub entries: Vec<CataloguedPlugin>,
    /// How many entries the index holds that could not be listed, for lacking a fact an
    /// entry carries. Present where it was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unreadable: Option<usize>,
    /// The setting that keeps the catalogue from being asked, and how to let it be.
    /// Present where it is switched off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setting: Option<ReachSetting>,
    /// Why the index could not be read. Present where it failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<CatalogueFailure>,
}

/// Whether the catalogue was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PluginCatalogueState {
    /// Its index was fetched and verified.
    Listed,
    /// Reaching it is switched off, and nothing was asked of it.
    Off,
    /// Its index or signature could not be fetched, verified or read.
    Failed,
}

/// One plugin the catalogue offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct CataloguedPlugin {
    /// The plugin's id, which is the name it is installed by.
    pub id: String,
    /// What it calls itself.
    pub name: String,
    /// What it does, in its author's words.
    pub description: String,
    /// The version the catalogue reviewed.
    pub version: String,
    /// What the household is without it.
    pub without_it: String,
    /// The version installed on this machine. Absent where it is not installed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed: Option<InstalledVersion>,
}

/// The version of a catalogued plugin installed here, against the catalogue's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct InstalledVersion {
    /// The version installed.
    pub version: String,
    /// Whether the catalogue's version is above it by semver.
    pub newer: bool,
}

/// A setting that keeps a request from being made, and the change that lets it be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ReachSetting {
    /// The setting.
    pub key: String,
    /// The value that lets the request be made.
    pub value: String,
    /// The action that writes it.
    pub action: SettingWrite,
}

/// The action a setting is written with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SettingWrite {
    /// `config-set`, which writes the configuration.
    ConfigSet,
}

/// Why the catalogue's index was not read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct CatalogueFailure {
    /// The problem code an install by name would be refused with.
    pub code: String,
    /// What went wrong, in one line.
    pub summary: String,
}
