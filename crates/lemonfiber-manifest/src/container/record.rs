//! The container fields an install record keeps for a service.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Certificate, Mount, Network, Posture, Publish, UserMode, Value};

/// The container fields an install record keeps for a service, beyond those every
/// record already holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ServiceFields")]
pub struct Fields {
    /// Its further published ports.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publish: Vec<Publish>,
    /// Whether its configuration directory is read-only.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub config_ro: bool,
    /// Its further mounts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mount: Vec<Mount>,
    /// Where the whole configuration directory of the service it fronts lands in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shares_config: Option<String>,
    /// The paths beneath its configuration directory it rebuilds when they are missing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rebuilt: Vec<String>,
    /// Its scratch paths in memory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tmpfs: Vec<String>,
    /// Its environment.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, Value>,
    /// Who it runs as, where declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_mode: Option<UserMode>,
    /// How tightly it is held.
    #[serde(default, skip_serializing_if = "Posture::is_standard")]
    pub posture: Posture,
    /// Its memory ceiling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem_limit_mib: Option<u32>,
    /// The network it runs inside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<Network>,
    /// The certificate lemonfiber makes for it, where it declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate: Option<Certificate>,
}

impl Fields {
    /// Whether it declares nothing beyond what every record holds.
    #[must_use]
    pub fn is_plain(&self) -> bool {
        *self == Self::default()
    }
}
