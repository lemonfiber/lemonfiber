//! Speaking NNTP to a Usenet provider, far enough to prove a login.
//!
//! A Usenet provider is reached not over HTTP but over NNTP, its own line
//! protocol: connect, read a greeting, authenticate with `AUTHINFO`, read the
//! reply. Only that much is needed to prove a credential — the codes the provider
//! answers with say whether the login took — so the port is one exchange rather
//! than a general client.
//!
//! Like the filesystem port, the decision lives above the seam: the adapter
//! speaks the wire and hands back the reply lines, and the caller reads the codes
//! into an outcome, so a fake can drive every branch of that reading without a
//! socket. The one crate that opens a real, TLS-wrapped connection stays confined
//! to a single adapter.

use async_trait::async_trait;
use thiserror::Error;

/// Where and how to reach a Usenet provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// The provider's hostname.
    pub host: String,
    /// The port it answers NNTP on — 563 for the usual TLS, 119 for plaintext.
    pub port: u16,
    /// Whether the connection is TLS-wrapped, as it must be to carry a password.
    pub secure: bool,
}

/// The provider could not be reached at all — a refused connection, a name that
/// did not resolve, a handshake that did not complete, a wait that ran out.
///
/// Distinct from any status the provider answers with: this is nothing usable
/// coming back, so nothing can be concluded about the credential.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("{host} could not be reached: {reason}")]
pub struct Unreachable {
    /// The host that was tried.
    pub host: String,
    /// The transport's own account of why.
    pub reason: String,
}

/// One authenticated NNTP exchange with a provider.
#[async_trait]
pub trait Nntp: Send + Sync {
    /// Connect to `endpoint`, read the greeting, send each command in turn reading
    /// its reply, and return the greeting followed by each reply — in order, so the
    /// caller can read the code the provider answered each step with.
    ///
    /// # Errors
    ///
    /// Returns [`Unreachable`] where nothing usable answered. A status line the
    /// provider sends — including a refusal — is a reply, not an error.
    async fn converse(
        &self,
        endpoint: &Endpoint,
        commands: &[String],
    ) -> Result<Vec<String>, Unreachable>;
}
