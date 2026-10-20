//! A guard in front of the media server: the declaration it enforces, the one proxy it
//! believes about a client, and where it asks the core about a live-stream pass.

use async_trait::async_trait;

use super::{Declared, Failure};

/// Everything a guard enforces, and nothing else.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Guarded {
    /// The media server's declaration, as its filler answered it.
    pub declared: Declared,
    /// Where the guard reaches the media server: its scheme, host and port.
    pub upstream: String,
    /// Where the guard reaches the media server filler's adapter, where it speaks the
    /// contract: its scheme, host and port.
    pub adapter: Option<String>,
    /// The one address it believes when a request says which client it came from.
    pub proxy: String,
    /// Where it asks the core whether a live-stream pass is good.
    pub pass: String,
}

/// The certificate a guard presents.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Presented {
    /// SHA-256 over the certificate's DER encoding, in lower-case hexadecimal.
    pub fingerprint: String,
}

/// A guard, told what it enforces and asked what it holds.
#[async_trait]
pub trait Guarding: Send + Sync {
    /// Enforce `guarded` in place of what it enforces now.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or refuses.
    async fn guard(&self, guarded: &Guarded) -> Result<(), Failure>;

    /// What it enforces now, or nothing where it was never told.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn guarding(&self) -> Result<Option<Guarded>, Failure>;

    /// The certificate it presents.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or presents none.
    async fn certificate(&self) -> Result<Presented, Failure>;
}
