//! Telling a service whether to link or copy when it imports.
//!
//! One field on one document, and the whole document is read before it is written:
//! the service replaces what it is sent, so writing the single field would
//! silently reset every other media-management setting — including ones the
//! operator chose themselves. A correction that quietly undoes unrelated choices
//! is worse than the fault it corrects.

use async_trait::async_trait;
use serde::Deserialize;

use super::Servarr;
use crate::ports::http::Method;
use crate::ports::service::{Failure, Importing};

#[async_trait]
impl Importing for Servarr {
    async fn hardlinks(&self) -> Result<bool, Failure> {
        let response = self
            .probe(&self.request(Method::Get, "/config/mediamanagement", None))
            .await?;
        let config: MediaManagement = self
            .endpoint
            .decode(&response, "the media-management settings could not be read")?;
        Ok(config.copy_using_hardlinks)
    }

    async fn set_hardlinks(&self, hardlink: bool) -> Result<(), Failure> {
        // Read first: the service replaces the whole document on a write, so
        // sending only the one field would silently reset every other setting on
        // it — including ones the operator chose themselves.
        let response = self
            .probe(&self.request(Method::Get, "/config/mediamanagement", None))
            .await?;
        let mut document: serde_json::Value = self
            .endpoint
            .decode(&response, "the media-management settings could not be read")?;
        let Some(fields) = document.as_object_mut() else {
            return Err(self
                .endpoint
                .refused("the media-management settings were not an object"));
        };
        fields.insert("copyUsingHardlinks".to_owned(), hardlink.into());

        let id = fields
            .get("id")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(1);
        let response = self
            .probe(&self.request(
                Method::Put,
                &format!("/config/mediamanagement/{id}"),
                Some(document.to_string()),
            ))
            .await?;
        self.endpoint.expect_success(&response)
    }
}

/// The media-management field that decides whether an import links or copies.
///
/// Named as Servarr sends it. The rest of the document is untouched: it carries
/// settings an operator may have chosen, and this correction is about one of them.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MediaManagement {
    #[serde(default)]
    copy_using_hardlinks: bool,
}
