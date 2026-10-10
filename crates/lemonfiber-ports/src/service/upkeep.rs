//! Keeping what a media server holds in order: how each series it holds is filed, and
//! asking it to read one item afresh.

use async_trait::async_trait;

use super::Failure;

/// The most series one reading of them asks for.
pub const SERIES_MOST: u32 = 5_000;

/// One series, with how many seasons and episodes the server files under it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct SeriesHeld {
    /// The identifier the server tells it apart by.
    pub id: String,
    /// What it is called, in the words the server holds it under.
    pub title: String,
    /// How many seasons the server answers for it.
    pub seasons: u32,
    /// How many episodes the server holds under it.
    pub episodes: u32,
}

/// How the series a media server holds are filed, and reading one item afresh.
#[async_trait]
pub trait Upkeep: Send + Sync {
    /// Every series the server holds, `most` of them, each with how many seasons it
    /// answers for it and how many episodes it holds under it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn series_held(&self, most: u32) -> Result<Vec<SeriesHeld>, Failure>;

    /// Ask the server to read one item, and everything filed under it, afresh.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn refresh(&self, id: &str) -> Result<(), Failure>;
}
