//! The quality capabilities a service answers — judging a resolution preset against what
//! the indexers carry, and applying an audio format to a service whose quality axis is a
//! format rather than a resolution. The D2 additions to the service port, kept apart from
//! the provisioning surface they accreted onto.

use async_trait::async_trait;

use super::Failure;

/// What a search for the operator's wanted content found, read against the quality
/// profile in force — the basis for telling "the preset yields no matching releases"
/// apart from "the indexer failed" (which surfaces as a [`Failure`], not a probe).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseProbe {
    /// Nothing is wanted, so there was nothing to search for — the preset cannot be
    /// judged against releases yet.
    NothingWanted,
    /// The search ran and returned releases the profile would grab — the preset is
    /// met by what is available.
    Matching,
    /// The search returned releases, but the profile wants none of them — they exist
    /// but do not meet the chosen quality, so the preset conflicts with what is out
    /// there.
    NoneMatch,
    /// The search ran cleanly and returned nothing at all — few or none are available,
    /// as distinct from the indexer having failed to answer.
    NoneFound,
}

/// Searching a resolution service for the operator's wanted content, to judge the
/// quality preset against what the indexers actually carry — Sonarr and Radarr, which
/// parse each release's quality and say whether their profile would grab it.
#[async_trait]
pub trait QualityReleases: Send + Sync {
    /// Search for one wanted item and read what came back against the profile. The
    /// item's id fills `id_param` (`episodeId` for television, `movieId` for film).
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service or its indexers are unreachable or refuse
    /// the search — the indexer-failure case, kept distinct from an empty result.
    async fn probe_releases(&self, id_param: &str) -> Result<ReleaseProbe, Failure>;
}

/// Applying an audio quality to a service that has one — Lidarr, whose quality axis is
/// a format rather than a resolution and which no community profile configures, so the
/// choice is carried straight to its own quality profiles.
#[async_trait]
pub trait MusicQuality: Send + Sync {
    /// Set every quality profile to the format, and — for a hi-res choice — ensure the
    /// 24-bit custom format exists and is preferred. Forward-looking, like every quality
    /// choice: it decides what is acquired next, not a rewrite of the library on disk.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses a change.
    async fn apply_music_format(&self, format: crate::media::Format) -> Result<(), Failure>;
}
