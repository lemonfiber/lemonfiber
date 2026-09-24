//! Stopping `SABnzbd` fetching, and starting it again.
//!
//! One request each way, and both halves of "stopped" in one: this client pauses
//! the downloader itself rather than the items in it, so what is running stops and
//! what arrives afterwards waits in the queue instead of starting. A Usenet client
//! has no second setting to reach for.
//!
//! Read back rather than believed, like every other write to a download client
//! here. This one answers a request it would not carry out with a `false` inside a
//! `200`, so even the request's own answer is read rather than assumed — and then
//! the queue is asked what it is actually doing.

use async_trait::async_trait;
use serde::Deserialize;

use crate::ports::service::{Failure, Fetching, Pulling};

use super::{Sabnzbd, Wrote};

/// `SABnzbd`'s `mode=queue` answer, read for whether it is fetching at all.
///
/// Its own shape rather than the one the rate is read through: that one wants the
/// two figures beside the slots, this one wants the flag above them, and a struct
/// holding both would oblige each read to carry the other's fields.
#[derive(Deserialize)]
struct Halted {
    queue: Paused,
}

/// Whether the downloader is paused, as the client reports it.
#[derive(Deserialize)]
struct Paused {
    #[serde(default)]
    paused: bool,
}

impl Sabnzbd {
    /// Ask for one thing and read what became of it, in one call.
    async fn asked(&self, mode: &str, whenever: &str) -> Result<(), Failure> {
        let wrote: Wrote = self.read(mode, whenever).await?;
        if wrote.status {
            return Ok(());
        }
        Err(self
            .endpoint
            .refused("the client answered that it would not"))
    }
}

#[async_trait]
impl Fetching for Sabnzbd {
    async fn pulling(&self) -> Result<Pulling, Failure> {
        let held: Halted = self
            .read("queue", "whether it is fetching could not be read")
            .await?;
        // One flag answers both halves here: a paused downloader neither moves what
        // it holds nor starts what arrives next.
        let fetching = !held.queue.paused;
        Ok(Pulling::of(fetching, fetching))
    }

    async fn stop(&self) -> Result<Pulling, Failure> {
        self.asked("pause", "the client could not be stopped")
            .await?;
        self.pulling().await
    }

    async fn resume(&self) -> Result<Pulling, Failure> {
        self.asked("resume", "the client could not be started")
            .await?;
        self.pulling().await
    }
}

#[cfg(test)]
mod tests;
