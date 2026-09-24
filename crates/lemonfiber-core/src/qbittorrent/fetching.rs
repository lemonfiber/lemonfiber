//! Stopping qBittorrent fetching, and starting it again.
//!
//! Two writes each way rather than one, because this client has no single switch
//! and the half that looks like one is not. Stopping every torrent stops what is
//! running; it does nothing about the next release an \*arr hands over, which the
//! client starts by itself within the hour. So the preference that adds new
//! torrents already stopped is written beside it, and the two together are what
//! "nothing new is fetched" comes to here.
//!
//! Both are read back, and the read-back is the same pair: nothing running, and
//! nothing new would start. A client answering `200` to both writes and going on
//! fetching is exactly the failure a cap exists to prevent.
//!
//! The endpoint names are this client's own and were taken from the pinned image
//! rather than from a document: at this web API version the pair is `stop` and
//! `start`, and the `pause` and `resume` that a great deal of writing still names
//! answer `404`.

use async_trait::async_trait;
use serde::Deserialize;

use crate::ports::service::{Failure, Fetching, Pulling};

use super::Qbittorrent;

/// Every torrent the client holds, addressed at once.
const EVERY: &str = "all";

/// The preference fields whether new work would start is read from. The many
/// others qBittorrent sends are ignored.
#[derive(Deserialize)]
struct Adding {
    #[serde(default)]
    add_stopped_enabled: bool,
}

/// One torrent the client reports as running.
///
/// Nothing is read out of it: how many came back is the whole answer, and naming a
/// field would tie this read to a shape it makes no use of.
#[derive(Deserialize)]
struct Running {}

impl Qbittorrent {
    /// How many torrents the client says are running.
    async fn running(&self) -> Result<usize, Failure> {
        let response = self.get("/torrents/info?filter=running").await?;
        let running: Vec<Running> = self
            .endpoint
            .decode(&response, "what the client is running could not be read")?;
        Ok(running.len())
    }

    /// Whether the client would start the next torrent handed to it.
    async fn would_start(&self) -> Result<bool, Failure> {
        let response = self.get("/app/preferences").await?;
        let adding: Adding = self
            .endpoint
            .decode(&response, "how new downloads are added could not be read")?;
        Ok(!adding.add_stopped_enabled)
    }

    /// Write the preference that decides what happens to the next torrent added.
    async fn adding_stopped(&self, stopped: bool) -> Result<(), Failure> {
        let asked = serde_json::json!({ "add_stopped_enabled": stopped }).to_string();
        let request = self.post("/app/setPreferences", &[("json", &asked)]);
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)
    }

    /// Stop or start every torrent the client holds.
    async fn every_torrent(&self, action: &str) -> Result<(), Failure> {
        let request = self.post(&format!("/torrents/{action}"), &[("hashes", EVERY)]);
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)
    }
}

#[async_trait]
impl Fetching for Qbittorrent {
    async fn pulling(&self) -> Result<Pulling, Failure> {
        self.signed_in().await?;
        Ok(Pulling::of(
            self.running().await? > 0,
            self.would_start().await?,
        ))
    }

    async fn stop(&self) -> Result<Pulling, Failure> {
        self.signed_in().await?;
        // The preference first. Stopping what is running and then leaving a window
        // in which the next grab starts would be a pause that let one more
        // download through every time it was asked for.
        self.adding_stopped(true).await?;
        self.every_torrent("stop").await?;
        self.pulling().await
    }

    async fn resume(&self) -> Result<Pulling, Failure> {
        self.signed_in().await?;
        self.adding_stopped(false).await?;
        self.every_torrent("start").await?;
        self.pulling().await
    }
}

#[cfg(test)]
mod tests;
