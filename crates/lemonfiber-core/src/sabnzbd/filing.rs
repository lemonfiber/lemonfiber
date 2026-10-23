//! Where `SABnzbd` files each category's completed downloads.
//!
//! A category's folder is its own `dir`: an absolute one is where its completed downloads
//! land, a relative one is read below the client's completed folder as the client resolves
//! it, and an empty one files them under the category's own name there. The category `*` is what a download
//! with no category is filed under, which no curator hands it, so it is not one of them.

use async_trait::async_trait;

use super::Sabnzbd;
use crate::endpoint::form_encoded;
use crate::ports::service::{Failure, Filed, Filing};

/// The category `SABnzbd` files a download no category was given under.
const UNCATEGORISED: &str = "*";

/// One `get_config` answer, as the sections read here carry it.
#[derive(serde::Deserialize)]
struct Held {
    config: Config,
}

/// The sections read here.
#[derive(serde::Deserialize)]
struct Config {
    #[serde(default)]
    categories: Vec<Category>,
}

/// One category and the folder it names.
#[derive(serde::Deserialize)]
struct Category {
    name: String,
    #[serde(default)]
    dir: String,
}

/// One `fullstatus` answer, as far as it is read here.
#[derive(serde::Deserialize)]
struct Status {
    status: Completing,
}

/// The client's completed folder, resolved against its own base folder.
#[derive(serde::Deserialize)]
struct Completing {
    completedir: String,
}

/// The client's answer to a category written, which carries a `false` status where it
/// refused one.
#[derive(serde::Deserialize)]
struct Answered {
    #[serde(default)]
    status: Option<bool>,
}

#[async_trait]
impl Filing for Sabnzbd {
    async fn filed(&self) -> Result<Vec<Filed>, Failure> {
        let held: Held = self
            .read(
                "get_config&section=categories",
                "the categories could not be read",
            )
            .await?;
        let completed: Status = self
            .read(
                "fullstatus&skip_dashboard=1",
                "the completed folder could not be read",
            )
            .await?;
        let below = completed.status.completedir;
        Ok(held
            .config
            .categories
            .into_iter()
            .filter(|category| category.name != UNCATEGORISED)
            .map(|category| Filed {
                folder: folder_of(&below, &category),
                category: category.name,
            })
            .collect())
    }

    async fn file_under(&self, folders: &[Filed]) -> Result<(), Failure> {
        file_under(self, folders).await
    }
}

async fn file_under(sabnzbd: &Sabnzbd, folders: &[Filed]) -> Result<(), Failure> {
    for one in folders {
        let mode = format!(
            "set_config&{}",
            form_encoded(&[
                ("section", "categories"),
                ("keyword", &one.category),
                ("dir", &one.folder),
            ])
        );
        let answered: Answered = sabnzbd
            .read(&mode, "a category's completed folder could not be set")
            .await?;
        if answered.status == Some(false) {
            return Err(sabnzbd.endpoint.refused(&format!(
                "the client would not file {} under {}",
                one.category, one.folder
            )));
        }
    }
    Ok(())
}

/// Where `category` files its completed downloads, below the client's completed folder
/// `below` where its own folder is not absolute.
fn folder_of(below: &str, category: &Category) -> String {
    let dir = category.dir.trim();
    let named = if dir.is_empty() {
        category.name.as_str()
    } else {
        dir
    };
    if named.starts_with('/') {
        return named.to_owned();
    }
    format!("{}/{named}", below.trim_end_matches('/'))
}

#[cfg(test)]
mod tests;
