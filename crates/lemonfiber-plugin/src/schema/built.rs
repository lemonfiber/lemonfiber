//! The upstream image a plugin's service image is built on.

use serde::{Deserialize, Serialize};

/// The upstream image a service's image is built on, and the release of it the build
/// started from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginBuiltOn")]
pub struct BuiltOn {
    /// The upstream image's registry path as the build names it, without tag or digest.
    pub image: String,
    /// The upstream's release the build started from, as that image tags it.
    pub release: String,
}
