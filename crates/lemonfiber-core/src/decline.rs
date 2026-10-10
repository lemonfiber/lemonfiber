//! Talking to the decline service, which answers the address an invitation is
//! declined at.
//!
//! The core asks it one thing: which key it holds. A rotation writes a new key where
//! the service reads it and revokes the old one only once the service says it holds the
//! new, so a decline is never left with a key the media server no longer takes.

use std::sync::Arc;

use lemonfiber_sidecar::decline::Health;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method};
use crate::ports::service::Failure;

/// A client for one decline service.
pub struct Decline {
    endpoint: Endpoint,
}

impl Decline {
    /// A client for the decline service reached at `base`.
    #[must_use]
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, "decline"),
        }
    }

    /// Which key the service holds, as its health route says.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the service is unreachable or answers with something
    /// unreadable.
    pub async fn health(&self) -> Result<Health, Failure> {
        let asking = self.endpoint.json_request(Method::Get, "/health", None);
        let response = self.endpoint.send(&asking).await?;
        self.endpoint
            .decode(&response, "its health could not be read")
    }
}

#[cfg(test)]
mod tests;
