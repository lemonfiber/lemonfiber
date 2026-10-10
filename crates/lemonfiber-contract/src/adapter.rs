//! What every adapter answers beside the capabilities it serves, and a capability as an
//! adapter kit serves it.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::Refusal;

/// The file in an adapter's configuration directory the core writes the plugin's key to.
pub const KEY_FILE: &str = "lemonfiber.key";

/// The file in an adapter's configuration directory the core writes its upstream's
/// credential to, as a JSON object of named strings.
pub const UPSTREAM_FILE: &str = "upstream.json";

/// What every adapter answers under, beside the capabilities it serves.
pub const ADAPTER: &str = "adapter";

/// The major of what every adapter answers.
pub const MAJOR: u32 = 1;

/// Where an adapter says what it is.
#[must_use]
pub fn about_path() -> String {
    crate::path(ADAPTER, MAJOR, "about")
}

/// Where an adapter says whether its upstream answers.
#[must_use]
pub fn ready_path() -> String {
    crate::path(ADAPTER, MAJOR, "ready")
}

/// What an adapter says it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct About {
    /// Each contract it speaks, as `capability@major`.
    pub speaks: Vec<String>,
    /// The upstream it fronts, as its project names itself.
    pub upstream: String,
    /// Every upstream release it supports.
    pub releases: Vec<Release>,
}

/// One upstream release an adapter supports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Release {
    /// The release, as upstream numbers it.
    pub version: String,
    /// The digest of the image its recordings were taken from.
    pub digest: String,
}

/// What a served call comes to.
pub type Answering = Pin<Box<dyn Future<Output = Result<Vec<u8>, Refusal>> + Send>>;

/// Serving one call: the operation and what it was asked with.
type Dispatching = dyn Fn(String, Vec<u8>) -> Answering + Send + Sync;

/// One capability's contract, served from an adapter's implementation of its ports.
#[derive(Clone)]
pub struct Served {
    /// The capability.
    pub capability: &'static str,
    /// The major served.
    pub major: u32,
    dispatching: Arc<Dispatching>,
}

impl std::fmt::Debug for Served {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Served")
            .field("capability", &self.capability)
            .field("major", &self.major)
            .finish_non_exhaustive()
    }
}

impl Served {
    /// `capability`'s `major`, each call served by `dispatching`.
    #[must_use]
    pub fn new(
        capability: &'static str,
        major: u32,
        dispatching: impl Fn(String, Vec<u8>) -> Answering + Send + Sync + 'static,
    ) -> Self {
        Self {
            capability,
            major,
            dispatching: Arc::new(dispatching),
        }
    }

    /// What it speaks, as `capability@major`.
    #[must_use]
    pub fn spoken(&self) -> String {
        crate::spoken(self.capability, self.major)
    }

    /// Serve `operation`, asked with `body`.
    ///
    /// # Errors
    ///
    /// The refusal the operation answered with.
    pub async fn dispatch(&self, operation: &str, body: Vec<u8>) -> Result<Vec<u8>, Refusal> {
        (self.dispatching)(operation.to_owned(), body).await
    }
}

#[cfg(test)]
mod tests;
