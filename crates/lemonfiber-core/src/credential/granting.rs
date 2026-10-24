//! The credentials granted to third-party plugins, and what revoking one came to.

use serde::Serialize;

/// A credential of another service granted to a third-party plugin, named by the setting it
/// is held under and never by its value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "CredentialGranted")]
pub struct Granted {
    /// The plugin it is granted to.
    pub plugin: String,
    /// The setting the credential is held under, as the inventory names it.
    pub credential: String,
    /// The service whose credential it is.
    pub of: String,
    /// The capability of the plugin's own it was granted to fill.
    #[serde(rename = "for")]
    pub serves: String,
    /// When the operator approved it.
    pub at: String,
}

impl Granted {
    /// `grant`, as the plugin `plugin` holds it.
    #[must_use]
    pub fn of(plugin: &str, grant: &crate::plugin::Grant) -> Self {
        Self {
            plugin: plugin.to_owned(),
            credential: grant.credential.clone(),
            of: grant.of.clone(),
            serves: grant.serves.clone(),
            at: grant.at.clone(),
        }
    }

    /// Every grant the plugins in `installed` hold, in the order they are recorded.
    #[must_use]
    pub fn every(installed: &[crate::plugin::Installed]) -> Vec<Self> {
        installed
            .iter()
            .flat_map(|one| one.granted.iter().map(|grant| Self::of(&one.plugin, grant)))
            .collect()
    }
}

/// What revoking one grant came to, or would come to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "CredentialRevoked")]
pub struct Revoked {
    /// The grant it ends.
    pub grant: Granted,
    /// The plugin's services that read the credential, restarted once it is withdrawn.
    pub restarts: Vec<String>,
    /// The name this reading goes by, which the revoke takes as its yes.
    pub agreement: String,
    /// Whether the grant was ended, rather than read or rehearsed.
    pub ended: bool,
    /// Why its services did not restart, where they did not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unrestarted: Option<String>,
}
