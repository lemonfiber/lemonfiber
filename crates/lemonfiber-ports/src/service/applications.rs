//! The services an indexer searches on behalf of.
//!
//! The one connection that runs outward from the indexer rather than into it.

use super::Failure;
use async_trait::async_trait;

/// Which media-filing \*arr a Prowlarr application entry syncs to, selecting the
/// field schema Prowlarr files it under and the release categories it syncs —
/// see the Prowlarr-application contract in the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationKind {
    /// Sonarr — television.
    Sonarr,
    /// Radarr — movies.
    Radarr,
    /// Lidarr — music.
    Lidarr,
}

/// A media-filing \*arr, as Prowlarr needs to be told about it so it syncs that
/// \*arr its indexers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Application {
    /// The name the operator will see in Prowlarr's own interface.
    pub name: String,
    /// Which application it is, selecting the field schema and sync categories.
    pub kind: ApplicationKind,
    /// The address the \*arr reaches Prowlarr back on, on the stack's network.
    pub prowlarr_url: String,
    /// The address Prowlarr reaches the \*arr on, on the stack's network — the
    /// connection an existing application is matched by.
    pub base_url: String,
    /// The \*arr's own API key, which is what lets Prowlarr write indexers into
    /// it.
    pub api_key: String,
}

/// An application Prowlarr already holds, with the identifier it gave it.
///
/// Read back so an application already registered can be told from an absent one
/// — matched by the address it reaches, the `base_url`, rather than by its label,
/// so a differently-named but equivalent application is not duplicated — and so a
/// later undo names exactly the one created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredApplication {
    /// The identifier Prowlarr assigned.
    pub id: String,
    /// The address it reaches the \*arr on.
    pub base_url: String,
}

/// Prowlarr's application sync — the one Servarr-shape service that manages other
/// Servarr applications rather than media.
///
/// It is a port of its own, not a method on [`Client`], because only Prowlarr has
/// applications and it versions its API a major behind the media \*arrs; a method
/// on the shared shape would be a capability the others do not have.
#[async_trait]
pub trait AppSync: Send + Sync {
    /// Tell Prowlarr about a media-filing \*arr, so it syncs that \*arr its
    /// indexers.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when Prowlarr is unreachable or refuses.
    async fn register_application(&self, application: &Application) -> Result<(), Failure>;

    /// The applications Prowlarr already holds, each by the address it reaches
    /// the \*arr on rather than its label.
    ///
    /// Read so an application already registered is left alone rather than
    /// duplicated, and so a registration can be confirmed by reading it back.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when Prowlarr is unreachable or refuses.
    async fn applications(&self) -> Result<Vec<RegisteredApplication>, Failure>;
}
