//! What crosses to a third-party plugin because the operator granted it.

use serde::{Deserialize, Serialize};

/// A credential of another service the operator granted a third-party plugin, named by
/// the setting it is held under and never by its value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginGrant")]
pub struct Grant {
    /// The setting the credential is held under, as the credentials read names it.
    pub credential: String,
    /// The service whose credential it is.
    pub of: String,
    /// The capability of the plugin's own it was granted to fill.
    #[serde(rename = "for")]
    pub serves: String,
    /// When the operator approved it, as the record stamps every change.
    pub at: String,
}

impl Grant {
    /// How the offer names it, and how the operator approves it: `<setting>@<plugin>`.
    #[must_use]
    pub fn approval(&self, plugin: &str) -> String {
        approval(&self.credential, plugin)
    }
}

/// How a grant of `credential` to `plugin` is named in an offer and approved.
#[must_use]
pub fn approval(credential: &str, plugin: &str) -> String {
    format!("{credential}@{plugin}")
}
