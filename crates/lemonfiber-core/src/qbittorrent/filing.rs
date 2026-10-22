//! Where the client files each category's completed downloads, as `download.torrent`
//! carries it.
//!
//! A category's save path is where its completed torrents land only while the client
//! manages torrents automatically; otherwise every torrent completes into the default
//! save path whatever its category. So filing a category switches automatic management
//! on, and the folder read back for each category is the one its torrents actually land
//! in: its own save path, the default one below its own name where it names none, or the
//! default save path itself while torrents are managed by hand.

use std::collections::BTreeMap;

use async_trait::async_trait;

use super::Qbittorrent;
use crate::ports::service::{Failure, Filed, Filing};

/// One category, as the client lists them.
#[derive(serde::Deserialize)]
struct Category {
    #[serde(rename = "savePath", default)]
    save_path: String,
}

/// The two preferences a category's folder depends on.
#[derive(serde::Deserialize)]
struct Saving {
    #[serde(default)]
    save_path: String,
    #[serde(default)]
    auto_tmm_enabled: bool,
}

#[async_trait]
impl Filing for Qbittorrent {
    async fn filed(&self) -> Result<Vec<Filed>, Failure> {
        let categories = self.categories().await?;
        let saving = self.saving().await?;
        Ok(categories
            .into_iter()
            .map(|(name, category)| Filed {
                folder: folder_of(&saving, &name, &category),
                category: name,
            })
            .collect())
    }

    async fn file_under(&self, folders: &[Filed]) -> Result<(), Failure> {
        let held = self.categories().await?;
        for one in folders {
            let path = if held.contains_key(&one.category) {
                "/torrents/editCategory"
            } else {
                "/torrents/createCategory"
            };
            let request = self.post(
                path,
                &[
                    ("category", one.category.as_str()),
                    ("savePath", one.folder.as_str()),
                ],
            );
            let response = self.endpoint.send(&request).await?;
            self.endpoint.expect_success(&response).map_err(|_| {
                self.endpoint.refused(&format!(
                    "the client would not file {} under {}",
                    one.category, one.folder
                ))
            })?;
        }
        if self.saving().await?.auto_tmm_enabled {
            return Ok(());
        }
        let managed = serde_json::json!({ "auto_tmm_enabled": true }).to_string();
        let request = self.post("/app/setPreferences", &[("json", &managed)]);
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)
    }
}

impl Qbittorrent {
    /// Every category the client holds, by name.
    async fn categories(&self) -> Result<BTreeMap<String, Category>, Failure> {
        let response = self.read("/torrents/categories").await?;
        self.endpoint
            .decode(&response, "the categories could not be read")
    }

    /// The default save path, and whether torrents are managed automatically.
    async fn saving(&self) -> Result<Saving, Failure> {
        let response = self.read("/app/preferences").await?;
        self.endpoint
            .decode(&response, "where torrents are saved could not be read")
    }
}

/// Where a torrent of category `name` completes into.
fn folder_of(saving: &Saving, name: &str, category: &Category) -> String {
    let default = saving.save_path.trim_end_matches('/');
    if !saving.auto_tmm_enabled {
        return default.to_owned();
    }
    match category.save_path.trim() {
        "" => format!("{default}/{name}"),
        own if own.starts_with('/') => own.trim_end_matches('/').to_owned(),
        own => format!("{default}/{own}"),
    }
}

#[cfg(test)]
mod tests;
