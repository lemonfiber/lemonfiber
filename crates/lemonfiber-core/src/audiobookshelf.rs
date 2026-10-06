//! Claiming the listening server before anybody else on the network can.
//!
//! Like Jellyfin, this one has no key to read: it starts with no account at all, and
//! the first visitor to it makes the root one. So the first run makes it, the same way
//! the media server's is made, and the credential lemonfiber keeps is the password it
//! minted rather than a key the service wrote down.

use std::sync::Arc;

use serde::Deserialize;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::Failure;

/// Where the first account is created — the only call this service answers before
/// one exists.
const INIT: &str = "/init";

/// Where the server says whether it has an account yet.
const STATUS: &str = "/status";

/// What the status says: whether a first account has been made.
#[derive(Deserialize)]
struct Status {
    #[serde(rename = "isInit", default)]
    is_init: bool,
}

/// A client for one Audiobookshelf.
pub struct Audiobookshelf {
    endpoint: Endpoint,
}

impl Audiobookshelf {
    /// A client for the server reached at `base`, named `service`.
    #[must_use]
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>, service: impl Into<String>) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, service),
        }
    }

    /// A JSON request to a path on the server.
    fn request(&self, method: Method, path: &str, body: Option<String>) -> Request {
        Request {
            method,
            url: self.endpoint.url(path),
            headers: crate::endpoint::json_content_type(body.as_ref())
                .into_iter()
                .collect(),
            body,
            pinned: None,
        }
    }

    /// Whether a first account already exists.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the server is unreachable or answers unreadably.
    pub async fn has_account(&self) -> Result<bool, Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, STATUS, None))
            .await?;
        let status: Status = self
            .endpoint
            .decode(&response, "the server's status could not be read")?;
        Ok(status.is_init)
    }

    /// Create the first account, which is what makes the server usable at all.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the server is unreachable or refuses. A server that
    /// already has one refuses this, which is why it is asked first.
    pub async fn create_account(&self, name: &str, password: &str) -> Result<(), Failure> {
        let body = serde_json::json!({ "newRoot": { "username": name, "password": password } })
            .to_string();
        let response = self
            .endpoint
            .send(&self.request(Method::Post, INIT, Some(body)))
            .await?;
        self.endpoint.expect_success(&response)
    }
}
