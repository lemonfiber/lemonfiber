//! What a plugin's service asks the stack for.

use serde::Deserialize;

/// One capability a plugin's service asks for.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginAsk")]
pub struct Ask {
    /// The plugin's own service that asks; optional where the plugin declares one.
    #[serde(default)]
    pub service: Option<String>,
    /// The core capability asked for.
    pub capability: String,
    /// Whether every service that fills it is reached rather than one.
    #[serde(default)]
    pub each: bool,
    /// The upstream's own API the service talks in, or every API it talks in any of,
    /// which the ask then reaches in place of the adapter fronting it.
    #[serde(default)]
    pub native: Option<lemonfiber_manifest::container::Apis>,
    /// The media, in the stack manifest's vocabulary, it asks a filler of; every filler
    /// where none are named.
    #[serde(default)]
    pub media_types: Option<Vec<String>>,
}
