//! Reading the library, and telling it to look again.
//!
//! The last question a trace has — is it finally visible and playable? — and the one
//! thing a walkthrough asks of the media server: that it go and look at what has just
//! arrived, rather than leaving the operator staring at a library that will notice in an
//! hour.

use async_trait::async_trait;
use serde::Deserialize;

use super::{item_type, Jellyfin};
use crate::ports::http::Method;
use crate::ports::service::{Failure, Library};
use crate::recyclarr::Kind;

/// A page of library items, as Jellyfin returns them under `Items`.
#[derive(Deserialize)]
struct ItemPage {
    #[serde(rename = "Items", default)]
    items: Vec<LibraryItem>,
}

/// The one field of a library item a trace reads: its title, to match the term against.
#[derive(Deserialize)]
struct LibraryItem {
    #[serde(rename = "Name", default)]
    name: String,
}

#[async_trait]
impl Library for Jellyfin {
    async fn has_item(&self, kind: Kind, term: &str) -> Result<bool, Failure> {
        // Narrow to the media type and recurse into every library folder, then match the
        // term in the same case-insensitive, contains way the *arr found the item by — so
        // the two ends of the trace agree on what "a title matches" means. The token, not
        // a query string, carries the term, so nothing here has to be URL-encoded.
        let path = format!("/Items?Recursive=true&IncludeItemTypes={}", item_type(kind));
        let request = self.as_admin(Method::Get, &path, None).await?;
        let response = self.endpoint.send(&request).await?;
        let page: ItemPage = self
            .endpoint
            .decode(&response, "the library could not be read")?;

        let needle = term.to_lowercase();
        Ok(page
            .items
            .iter()
            .any(|item| item.name.to_lowercase().contains(&needle)))
    }

    async fn rescan(&self) -> Result<(), Failure> {
        // Every library, not the one the item landed in: Jellyfin's refresh is per-server
        // and the alternative is guessing which of the household's folders an *arr filed
        // something under, which is a guess with nothing riding on being right.
        let request = self
            .as_admin(Method::Post, "/Library/Refresh", None)
            .await?;
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)
    }
}
