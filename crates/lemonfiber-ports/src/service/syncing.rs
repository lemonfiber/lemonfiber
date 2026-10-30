//! A quality sync: the preset it holds a curator's quality profiles to.

use async_trait::async_trait;

use super::{Applied, Failure};
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

/// A hand-edited quality sync configuration and what putting the profiles back writes in
/// its place, both whole, as the sync answers them: unmasked, so read by nothing before
/// the core has masked each.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Edited {
    /// The configuration file, as the upstream's configuration directory names it.
    pub path: String,
    /// The whole of what is on disk.
    pub on_disk: String,
    /// The whole of what putting the profiles back writes in its place.
    pub preset: String,
}

impl std::fmt::Debug for Edited {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Edited")
            .field("path", &self.path)
            .field("on_disk", &format_args!("{} bytes", self.on_disk.len()))
            .field("preset", &format_args!("{} bytes", self.preset.len()))
            .finish()
    }
}

/// What putting the profiles back over a hand edit came to.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Reapplied {
    /// The configuration it replaced and what it wrote, or none where the configuration
    /// was the adapter's own.
    pub replaced: Option<Edited>,
    /// When the profiles are in force.
    pub applied: Applied,
}

/// A quality sync, told the preset each curator's profiles are held to.
#[async_trait]
pub trait Syncing: Send + Sync {
    /// Hold the curator of `profiled.which` to `profiled.preset`, at once or once it
    /// restarts.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or refuses.
    async fn apply(&self, profiled: &Profiled) -> Result<Applied, Failure>;

    /// The preset it last applied to the curator of `which`, or none.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn applied(&self, which: Kind) -> Result<Option<Preset>, Failure>;

    /// The hand-edited configuration putting `profiled` back would replace and what it
    /// would write in its place, or none, with nothing written.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn edited(&self, profiled: &Profiled) -> Result<Option<Edited>, Failure>;

    /// Hold the curator of `profiled.which` to `profiled.preset` over a hand edit, and
    /// the configuration it replaced and what it wrote.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or refuses.
    async fn reapply(&self, profiled: &Profiled) -> Result<Reapplied, Failure>;
}
