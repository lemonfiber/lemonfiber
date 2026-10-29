//! The extractor that unpacks what lands in each curator's download folder and hands it
//! back to that curator itself.

use async_trait::async_trait;

use super::{Applied, Failure};

/// One curator the extractor watches the download folder of, and hands back to.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Curated {
    /// The media type it files.
    pub media_type: String,
    /// Where the extractor reaches it.
    pub address: String,
    /// Its own key.
    pub key: String,
    /// The folder its downloads land in, as the extractor sees it.
    pub folder: String,
}

impl std::fmt::Debug for Curated {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Curated")
            .field("media_type", &self.media_type)
            .field("address", &self.address)
            .field("folder", &self.folder)
            .finish_non_exhaustive()
    }
}

/// One download it is extracting now.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Extraction {
    /// The download's name.
    pub name: String,
    /// The media type of the curator it is handed back to.
    pub media_type: String,
}

/// An extractor, told whose downloads to watch.
#[async_trait]
pub trait Extracting: Send + Sync {
    /// Watch `curators` in place of every curator it watches now, at once or once it
    /// restarts.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or refuses.
    async fn watch(&self, curators: &[Curated]) -> Result<Applied, Failure>;

    /// What it is extracting now.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn working(&self) -> Result<Vec<Extraction>, Failure>;
}

#[cfg(test)]
mod tests;
