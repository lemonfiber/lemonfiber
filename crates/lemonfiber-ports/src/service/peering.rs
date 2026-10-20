//! The port a torrent client takes peers on, which is the one its egress guard's provider
//! forwards where it forwards one.

use async_trait::async_trait;

use super::Failure;

/// A torrent client, asked and told the port it takes peers on.
#[async_trait]
pub trait Peering: Send + Sync {
    /// The port it takes peers on, or none where it is set to no forwarded port.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn peer_port(&self) -> Result<Option<u16>, Failure>;

    /// Take peers on `port`, or on no forwarded port where `port` is none, and confirm it
    /// took it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable, refuses, or answers another port
    /// afterwards.
    async fn set_peer_port(&self, port: Option<u16>) -> Result<(), Failure>;
}
