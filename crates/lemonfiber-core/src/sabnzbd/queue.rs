//! What `SABnzbd` is downloading right now.
//!
//! The queue answer and the figures inside it, which `SABnzbd` writes as strings in
//! its own dialect — megabytes stepped by 1024, a rate in kilobytes, a remaining time
//! as `HH:MM:SS`. Translating those into the plain bytes and seconds a [`Download`]
//! carries is this half's whole job, kept apart from the accounts half because the two
//! share only the client they are asked through.

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use crate::ports::service::{Download, Failure, Transfers};

use super::{Sabnzbd, DOWNLOADING};

/// `SABnzbd`'s `mode=queue` answer: the queue, and inside it the slots and the one
/// speed the whole queue downloads at.
#[derive(Deserialize)]
pub(crate) struct QueueResponse {
    queue: Queue,
}

/// The queue as `SABnzbd` reports it. `kbpersec` is the download rate for the
/// whole queue — `SABnzbd` downloads one item at a time — so it belongs to
/// whichever slot is the active one.
#[derive(Deserialize)]
pub(crate) struct Queue {
    #[serde(default)]
    kbpersec: String,
    #[serde(default)]
    slots: Vec<Slot>,
}

/// One queued download as `SABnzbd` reports it, every figure a string.
#[derive(Deserialize)]
pub(crate) struct Slot {
    filename: String,
    percentage: String,
    status: String,
    timeleft: String,
    #[serde(default)]
    mbleft: String,
}

#[async_trait]
impl Transfers for Sabnzbd {
    async fn transfers(&self) -> Result<Vec<Download>, Failure> {
        let body: QueueResponse = self.read("queue", "the queue could not be read").await?;
        let active_speed = bytes_per_second(&body.queue.kbpersec);
        Ok(body
            .queue
            .slots
            .into_iter()
            .map(|slot| download_of(slot, active_speed))
            .collect())
    }
}

/// One slot as the dashboard's [`Download`]. The queue's single speed is the
/// active slot's; every other slot is not moving, so it reads a definite zero
/// rather than the queue's rate or an unknown.
pub(crate) fn download_of(slot: Slot, active_speed: Option<u64>) -> Download {
    let speed = if slot.status == DOWNLOADING {
        active_speed
    } else {
        Some(0)
    };
    Download {
        name: slot.filename,
        progress: slot.percentage.trim().parse::<u8>().unwrap_or(0).min(100),
        speed,
        eta: seconds_left(&slot.timeleft)
            .filter(|left| *left > 0)
            .map(Duration::from_secs),
        remaining: bytes_left(&slot.mbleft),
    }
}

/// `SABnzbd`'s `mbleft` — a decimal string of megabytes still to fetch — as bytes,
/// or `None` where it will not parse. The whole-megabyte part is taken, matching
/// [`bytes_per_second`]'s reasoning: sub-megabyte precision is far below the
/// gigabyte scale the free-space projection weighs, so carrying the fraction would
/// need a float cast for no difference the operator could see.
pub(crate) fn bytes_left(mbleft: &str) -> Option<u64> {
    let whole = mbleft.split_once('.').map_or(mbleft, |(whole, _)| whole);
    whole
        .trim()
        .parse::<u64>()
        .ok()
        .map(|mb| mb.saturating_mul(1024 * 1024))
}

/// `SABnzbd`'s `kbpersec` — a decimal string of kilobytes per second — as bytes
/// per second, or `None` where it will not parse. The whole-kilobyte part is
/// taken; sub-kilobyte precision is below anything the dashboard shows, so
/// carrying the fraction would need a float cast for no visible difference.
pub(crate) fn bytes_per_second(kbpersec: &str) -> Option<u64> {
    let whole = kbpersec
        .split_once('.')
        .map_or(kbpersec, |(whole, _)| whole);
    whole
        .trim()
        .parse::<u64>()
        .ok()
        .map(|kb| kb.saturating_mul(1024))
}

/// A `SABnzbd` `timeleft` — colon-separated `H:MM:SS`, the hours unbounded — as
/// whole seconds, or `None` where any field will not parse. Every field is base
/// sixty, so folding by sixty is exact however many there are; an empty string is
/// `None` rather than zero.
pub(crate) fn seconds_left(timeleft: &str) -> Option<u64> {
    if timeleft.is_empty() {
        return None;
    }
    let mut total: u64 = 0;
    for field in timeleft.split(':') {
        let value: u64 = field.trim().parse().ok()?;
        total = total.saturating_mul(60).saturating_add(value);
    }
    Some(total)
}
