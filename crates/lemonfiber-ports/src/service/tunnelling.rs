//! The egress guard every download inside it leaves through: whether its tunnel is up,
//! and the port its provider forwards to it now.

use async_trait::async_trait;

use super::Failure;

/// An egress guard, asked about its tunnel.
#[async_trait]
pub trait Tunnelling: Send + Sync {
    /// Whether its tunnel is up.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or cannot tell.
    async fn up(&self) -> Result<bool, Failure>;

    /// The port its provider forwards to it now, or none.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or cannot tell.
    async fn forwarded(&self) -> Result<Option<u16>, Failure>;
}
