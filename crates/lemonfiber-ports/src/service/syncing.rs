//! A quality sync: the preset it holds a curator's quality profiles to.

use async_trait::async_trait;

use super::Failure;
use crate::media::preset::Preset;
use crate::media::Kind;

/// One curator, and the preset its profiles are held to.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Profiled {
    /// What the curator files.
    pub which: Kind,
    /// The host the sync reaches the curator on.
    pub host: String,
    /// The port the curator listens on.
    pub port: u16,
    /// The curator's own key.
    pub api_key: String,
    /// The preset its profiles are held to.
    pub preset: Preset,
}

/// A quality sync, told the preset each curator's profiles are held to.
#[async_trait]
pub trait Syncing: Send + Sync {
    /// Hold the curator of `profiled.which` to `profiled.preset`.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or refuses.
    async fn apply(&self, profiled: &Profiled) -> Result<(), Failure>;

    /// The preset it last applied to the curator of `which`, or none.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn applied(&self, which: Kind) -> Result<Option<Preset>, Failure>;
}
