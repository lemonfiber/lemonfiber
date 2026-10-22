//! The folder a download client files each category's completed downloads under.

use async_trait::async_trait;

use super::Failure;

/// One category a download client files completed downloads under, and the folder they
/// complete into.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Filed {
    /// The category, as a curator names it on a download it hands the client.
    pub category: String,
    /// The folder a completed download of that category lands in, as the client's
    /// container sees it.
    pub folder: String,
}

/// A download client, asked and told where each category's completed downloads land.
#[async_trait]
pub trait Filing: Send + Sync {
    /// Every category it holds and the completed folder it files each under.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn filed(&self) -> Result<Vec<Filed>, Failure>;

    /// File each category's completed downloads under the folder beside it, making a
    /// category it does not hold yet.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or refuses one of them.
    async fn file_under(&self, folders: &[Filed]) -> Result<(), Failure>;
}
