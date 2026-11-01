//! A media server, given a library for each media type the stack files.

use async_trait::async_trait;

use super::Failure;

/// One media type the stack files, and the folder the media server reads it at.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct FiledAt {
    /// The media type, in the stack manifest's vocabulary.
    pub media_type: String,
    /// The folder holding it, as a path inside the media server's container.
    pub folder: String,
}

/// Whether a library of the media server now reads one media type.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Furnished {
    /// The media type, as it was asked.
    pub media_type: String,
    /// Whether a library reads its folder.
    pub held: bool,
}

/// A media server, given a library for each media type the stack files, never changing
/// one the operator arranged.
#[async_trait]
pub trait Furnishing: Send + Sync {
    /// Add a library for each of `filed` whose folder exists and that no library reads,
    /// renaming, re-pointing and removing none, and answer for each whether a library
    /// now reads it; a media type the server does not take is answered not held.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unsupported`] where its upstream cannot be given a library, and
    /// [`Failure`] where it is unreachable or refuses.
    async fn furnish(&self, filed: &[FiledAt]) -> Result<Vec<Furnished>, Failure>;
}
