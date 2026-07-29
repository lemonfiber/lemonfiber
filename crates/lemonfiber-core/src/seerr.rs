//! Configuring Seerr to authenticate against Jellyfin.
//!
//! Seerr (Jellyseerr) is initialised by its first authenticated call: the first
//! account to sign in through Jellyfin becomes its owner, and the media server is
//! set to that Jellyfin at the same time. So this reads whether it is already
//! initialised — which lemonfiber never overrides, since re-pointing a running
//! instance's identity would cost the household its existing sign-ins — and, where
//! it is not, signs in through Jellyfin to point Seerr's authentication at it.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::{Failure, Requests};

/// The address a fresh Seerr owner is filed under. Seerr requires an address on
/// the initialising sign-in but derives the real one from Jellyfin, so a stable
/// placeholder in lemonfiber's own domain is enough.
const OWNER_EMAIL: &str = "admin@lemonfiber.local";

/// Selects Jellyfin as the media server, as Seerr numbers its kinds — Jellyfin,
/// not Plex or Emby.
const JELLYFIN_SERVER_TYPE: u8 = 2;

/// A client for one Seerr's identity setup.
pub struct Seerr {
    endpoint: Endpoint,
}

impl Seerr {
    /// A client for the Seerr reached at `base`, named `service`.
    #[must_use]
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>, service: impl Into<String>) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, service),
        }
    }

    /// A request to a path under Seerr's versioned API. A JSON body is declared as
    /// such, because Seerr's framework only parses a body it is told is JSON and
    /// silently drops one it is not.
    fn request(&self, method: Method, path: &str, body: Option<String>) -> Request {
        let headers = json_headers(body.as_ref());
        Request {
            method,
            url: self.endpoint.url(&format!("/api/v1{path}")),
            headers,
            body,
        }
    }
}

/// The `Content-Type` a request carries: JSON where it has a body, nothing where
/// it does not.
fn json_headers(body: Option<&String>) -> Vec<(String, String)> {
    if body.is_some() {
        vec![("Content-Type".to_owned(), "application/json".to_owned())]
    } else {
        Vec::new()
    }
}

/// The one field of the public settings lemonfiber reads: whether Seerr has been
/// initialised.
#[derive(Deserialize)]
struct PublicSettings {
    #[serde(default)]
    initialized: bool,
}

#[async_trait]
impl Requests for Seerr {
    async fn initialized(&self) -> Result<bool, Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, "/settings/public", None))
            .await?;
        let settings: PublicSettings = self
            .endpoint
            .decode(&response, "the public settings could not be read")?;
        Ok(settings.initialized)
    }

    async fn configure_identity(
        &self,
        username: &str,
        password: &str,
        server_url: &str,
    ) -> Result<(), Failure> {
        // Signing in through Jellyfin creates the owner and sets the media server,
        // but it does not finish setup — Seerr still reports itself uninitialised.
        let body = serde_json::json!({
            "username": username,
            "password": password,
            "hostname": server_url,
            "email": OWNER_EMAIL,
            "serverType": JELLYFIN_SERVER_TYPE,
        })
        .to_string();
        let signed_in = self
            .endpoint
            .send(&self.request(Method::Post, "/auth/jellyfin", Some(body)))
            .await?;
        if !signed_in.is_success() {
            return Err(self.endpoint.refusal(&signed_in));
        }

        // Finishing setup is the step that marks Seerr initialised. The session
        // cookie the sign-in set, carried by the transport, is what authorises it.
        let finished = self
            .endpoint
            .send(&self.request(Method::Post, "/settings/initialize", None))
            .await?;
        if finished.is_success() {
            Ok(())
        } else {
            Err(self.endpoint.refusal(&finished))
        }
    }
}
