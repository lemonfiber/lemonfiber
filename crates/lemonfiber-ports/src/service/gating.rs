//! The gate the request service reaches the curators and the media server through: the
//! routes it admits, the credential it presents on each, and the tokens it admits there.

use async_trait::async_trait;

use super::Failure;

/// What a route reaches.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Target {
    /// A curator.
    Curator {
        /// The media type it files.
        media_type: String,
    },
    /// The media server.
    MediaServer,
}

/// One route the gate admits, as it is applied.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Gated {
    /// What it reaches.
    pub target: Target,
    /// Where the gate reaches it.
    pub address: String,
    /// The credential the gate presents to it.
    pub credential: String,
    /// The SHA-256 of each token the gate admits on it, in lower-case hexadecimal.
    pub tokens: Vec<String>,
}

impl std::fmt::Debug for Gated {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Gated")
            .field("target", &self.target)
            .field("address", &self.address)
            .field("tokens", &self.tokens)
            .finish_non_exhaustive()
    }
}

/// One route the gate admits, as it answers it: its credential by fingerprint alone.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Admitted {
    /// What it reaches.
    pub target: Target,
    /// Where the gate reaches it.
    pub address: String,
    /// The credential it presents, by its fingerprint: `~` and the low sixteen bits of the 64-bit FNV-1a hash of its bytes, as four lower-case hexadecimal digits.
    pub fingerprint: String,
    /// The SHA-256 of each token it admits on it, in lower-case hexadecimal.
    pub tokens: Vec<String>,
}

impl Admitted {
    /// `gated`, as a gate holding it answers it.
    #[must_use]
    pub fn of(gated: &Gated) -> Self {
        Self {
            target: gated.target.clone(),
            address: gated.address.clone(),
            fingerprint: crate::fingerprint(&gated.credential),
            tokens: gated.tokens.clone(),
        }
    }
}

/// A request gate, told every route it admits.
#[async_trait]
pub trait Gating: Send + Sync {
    /// Admit `routes` in place of every route it admits now, and only their tokens.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or refuses.
    async fn admit(&self, routes: &[Gated]) -> Result<(), Failure>;

    /// Every route it admits now.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn admitted(&self) -> Result<Vec<Admitted>, Failure>;
}

#[cfg(test)]
mod tests;
