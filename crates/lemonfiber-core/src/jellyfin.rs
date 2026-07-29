//! Driving Jellyfin's first-run setup.
//!
//! Jellyfin is the one service lemonfiber sets an account on rather than reading
//! a key from: it writes no key to disk and asks for its first account through a
//! setup wizard whose endpoints answer only until setup completes. So this reads
//! whether the wizard is done and, where it is not, creates the administrator
//! with a password lemonfiber minted and finishes the wizard — after which those
//! endpoints stop answering, which is what makes the whole thing safe to drive
//! exactly once.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::{Failure, MediaServer};

/// A client for one Jellyfin's setup.
pub struct Jellyfin {
    endpoint: Endpoint,
}

impl Jellyfin {
    /// A client for the Jellyfin reached at `base`, named `service`.
    #[must_use]
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>, service: impl Into<String>) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, service),
        }
    }

    /// A request to a path on Jellyfin. The setup endpoints are unauthenticated —
    /// there is no key yet, which is the whole reason lemonfiber is here — and a
    /// JSON body is declared as such so it is bound rather than refused.
    fn request(&self, method: Method, path: &str, body: Option<String>) -> Request {
        let headers = if body.is_some() {
            vec![("Content-Type".to_owned(), "application/json".to_owned())]
        } else {
            Vec::new()
        };
        Request {
            method,
            url: self.endpoint.url(path),
            headers,
            body,
        }
    }
}

/// The one field of the public system info lemonfiber reads: whether the setup
/// wizard has already run. Named as Jellyfin sends it, in `PascalCase`.
#[derive(Deserialize)]
struct PublicInfo {
    #[serde(rename = "StartupWizardCompleted", default)]
    startup_wizard_completed: bool,
}

#[async_trait]
impl MediaServer for Jellyfin {
    async fn startup_completed(&self) -> Result<bool, Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, "/System/Info/Public", None))
            .await?;
        let info: PublicInfo = self
            .endpoint
            .decode(&response, "the public system info could not be read")?;
        Ok(info.startup_wizard_completed)
    }

    async fn create_admin(&self, name: &str, password: &str) -> Result<(), Failure> {
        let body = serde_json::json!({ "Name": name, "Password": password }).to_string();
        let created = self
            .endpoint
            .send(&self.request(Method::Post, "/Startup/User", Some(body)))
            .await?;
        if !created.is_success() {
            return Err(self.endpoint.refusal(&created));
        }

        let completed = self
            .endpoint
            .send(&self.request(Method::Post, "/Startup/Complete", None))
            .await?;
        if completed.is_success() {
            Ok(())
        } else {
            Err(self.endpoint.refusal(&completed))
        }
    }
}
