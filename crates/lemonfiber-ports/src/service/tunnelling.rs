//! The egress guard every download inside it leaves through: whether its tunnel is up,
//! and the port its provider forwards to it now.

use async_trait::async_trait;

use super::{Applied, Failure};

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

    /// Hold its control server to the credential its adapter was handed, at once or once
    /// it restarts.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or refuses.
    async fn secure(&self) -> Result<Applied, Failure>;
}
