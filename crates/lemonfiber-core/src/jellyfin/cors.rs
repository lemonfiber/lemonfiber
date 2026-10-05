//! Which origins a browser may read Jellyfin from.
//!
//! Jellyfin answers every origin until its allow-list names some, and reads an empty
//! list as every origin too — measured on each supported line. So the list is written
//! whole, as one origin, and never as nothing: the one origin a household's browser
//! reaches Jellyfin from is the front door's, because its own web client is served
//! from its own origin and every other service reaches it from the server side.
//!
//! The list lives in the server's configuration, which is written back whole or not at
//! all, so the configuration is read, the one field changed, and the whole of it
//! returned — then read again, because a write the server accepted and did not keep is
//! the failure this exists to catch.

use crate::ports::http::Method;
use crate::ports::service::Failure;

use super::Jellyfin;

/// Where the server's configuration is read and written.
const CONFIGURATION: &str = "/System/Configuration";

/// The field that holds the cross-origin allow-list.
const CORS_HOSTS: &str = "CorsHosts";

impl Jellyfin {
    /// The origins the allow-list names now.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where Jellyfin is unreachable, refuses the sign-in, or answers
    /// with a configuration that cannot be read.
    pub async fn cors_hosts(&self) -> Result<Vec<String>, Failure> {
        let configuration = self.configuration().await?;
        Ok(hosts(&configuration))
    }

    /// Name `origin` alone in the allow-list, and hold the server to having kept it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the origin is empty or a wildcard, where Jellyfin is
    /// unreachable, refuses the sign-in or the write, or reads back a list other than
    /// the one written.
    pub async fn allow_only(&self, origin: &str) -> Result<(), Failure> {
        if origin.trim().is_empty() || origin.contains('*') {
            return Err(self.endpoint.refused(&format!(
                "{origin:?} is not an origin the allow-list may be written as"
            )));
        }
        let mut configuration = self.configuration().await?;
        let Some(fields) = configuration.as_object_mut() else {
            return Err(self
                .endpoint
                .refused("the server configuration is not an object"));
        };
        fields.insert(CORS_HOSTS.to_owned(), serde_json::json!([origin]));
        let response = self
            .as_admin(Method::Post, CONFIGURATION, Some(configuration.to_string()))
            .await?;
        self.endpoint.expect_success(&response)?;
        let kept = self.cors_hosts().await?;
        if kept != [origin] {
            return Err(self.endpoint.refused(&format!(
                "the allow-list was written as {origin} and reads back as {kept:?}"
            )));
        }
        Ok(())
    }

    /// The server's whole configuration, as it answers it.
    async fn configuration(&self) -> Result<serde_json::Value, Failure> {
        let response = self.as_admin(Method::Get, CONFIGURATION, None).await?;
        self.endpoint
            .decode(&response, "the server configuration could not be read")
    }
}

/// The origins a configuration's allow-list names, in the order it names them.
fn hosts(configuration: &serde_json::Value) -> Vec<String> {
    configuration
        .get(CORS_HOSTS)
        .and_then(serde_json::Value::as_array)
        .map(|listed| {
            listed
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}
