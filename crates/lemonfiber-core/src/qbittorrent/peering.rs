//! The port the client takes peers on, as `download.torrent` carries it.

use async_trait::async_trait;

use super::Qbittorrent;
use crate::ports::service::{Failure, Peering};

/// The port the client is set to where no port is forwarded, as the stack's own push
/// sets it when forwarding ends.
const UNFORWARDED: u16 = 0;

#[async_trait]
impl Peering for Qbittorrent {
    async fn peer_port(&self) -> Result<Option<u16>, Failure> {
        let port = self.listen_port().await?;
        Ok((port != UNFORWARDED).then_some(port))
    }

    async fn set_peer_port(&self, port: Option<u16>) -> Result<(), Failure> {
        self.set_listen_port(port.unwrap_or(UNFORWARDED)).await
    }
}

#[cfg(test)]
mod tests;
