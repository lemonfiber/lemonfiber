//! The services an indexer searches on behalf of.
//!
//! The one connection that runs outward from the indexer rather than into it.

use super::Failure;
use crate::media::Kind;
use async_trait::async_trait;

/// What an application the indexer searches for files, selecting how the indexer
/// files the application and which release categories it syncs to it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationKind {
    /// Television.
    Tv,
    /// Film.
    Movies,
    /// Music.
    Music,
}

impl ApplicationKind {
    /// Every kind, in the order they are offered.
    pub const ALL: [Self; 3] = [Self::Tv, Self::Movies, Self::Music];

    /// The media type a service declares for this kind.
    #[must_use]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Tv => Kind::Tv.media_type(),
            Self::Movies => Kind::Movies.media_type(),
            Self::Music => crate::media::MUSIC,
        }
    }

    /// The kind a declared media type names, or `None` for one no application files.
    #[must_use]
    pub fn for_media_type(media_type: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.media_type() == media_type)
    }
}

/// A media-filing curator, as an indexer manager needs to be told about it so it syncs that
/// curator its indexers.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Application {
    /// The name the operator will see in the indexer manager's own interface.
    pub name: String,
    /// Which application it is, selecting the field schema and sync categories.
    pub kind: ApplicationKind,
    /// The address the application reaches the indexer back on, on the stack's network.
    pub indexer_url: String,
    /// The address the indexer manager reaches the curator on, on the stack's network — the
    /// connection an existing application is matched by.
    pub base_url: String,
    /// The curator's own API key, which is what lets the indexer manager write indexers into
    /// it.
    pub api_key: String,
}

/// An application the indexer manager already holds, with the identifier it gave it.
///
/// Read back so an application already registered can be told from an absent one
/// — matched by the address it reaches, the `base_url`, rather than by its label,
/// so a differently-named but equivalent application is not duplicated — and so a
/// later undo names exactly the one created.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RegisteredApplication {
    /// The identifier the indexer manager assigned.
    pub id: String,
    /// The address it reaches the curator on.
    pub base_url: String,
}

/// An indexer manager's application sync: the curators it hands its indexers to.
#[async_trait]
pub trait AppSync: Send + Sync {
    /// Tell the indexer manager about a media-filing curator, so it syncs that curator its
    /// indexers.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the indexer manager is unreachable or refuses.
    async fn register_application(&self, application: &Application) -> Result<(), Failure>;

    /// The applications the indexer manager already holds, each by the address it reaches
    /// the curator on rather than its label.
    ///
    /// Read so an application already registered is left alone rather than
    /// duplicated, and so a registration can be confirmed by reading it back.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the indexer manager is unreachable or refuses.
    async fn applications(&self) -> Result<Vec<RegisteredApplication>, Failure>;

    /// Whether the indexer manager reaches the curator an application it holds names, with what
    /// that application holds — the test its own settings page runs.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the indexer manager is unreachable, or the test does not pass.
    async fn test_application(&self, held: &RegisteredApplication) -> Result<(), Failure>;

    /// Give an application the indexer manager holds `key` for the curator it names, leaving
    /// everything else about it as the indexer manager holds it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the indexer manager is unreachable, refuses, or no longer holds it.
    async fn rekey_application(
        &self,
        held: &RegisteredApplication,
        key: &str,
    ) -> Result<(), Failure>;
}
