//! Holding `SABnzbd` to a share of the line, on the household's own hours.
//!
//! Two things are true of this client and not of the torrent one.
//!
//! **It does not upload.** Usenet is a download and nothing else, so an upload
//! limit on it is not a limit it ignored — it is a limit with nothing to apply to,
//! and the port carries the difference so a report can say which it met.
//!
//! **Its schedule is a list rather than a window.** qBittorrent keeps two sets of
//! limits and switches between them; this client keeps dated instruction lines and
//! switches the one limit it has. So the household's day becomes two lines — the
//! active rate at the hour people get up, no limit at the hour they stop — written
//! through [`super::scheduling`], which leaves every line the operator wrote alone.
//!
//! A window and a standing limit are the same setting reached two ways, so only one
//! of them is ever in force: with a window the client's own scheduler owns the rate
//! and nothing here writes it directly, and without one the rate is written directly
//! and the schedule is emptied of anything that would move it.

use async_trait::async_trait;
use serde::Deserialize;

use crate::ports::service::{Failure, Rates, Throttled, Throttling, Wanted};

use super::queue::bytes_per_second;
use super::scheduling::{side, Turn};
use super::{Sabnzbd, Wrote};

/// `SABnzbd`'s `mode=queue` answer, read for the limit rather than the slots.
///
/// A separate shape from the one the transfers read uses: that one wants the
/// slots, this one wants the two figures beside them, and a struct holding both
/// would oblige each read to carry the other's fields.
#[derive(Deserialize)]
struct Limited {
    queue: Held,
}

/// The limit the client is under, and what it is doing beneath it.
///
/// `speedlimit_abs` is the limit in bytes a second, written as a string like every
/// other figure this client sends, and empty where nothing holds it back.
#[derive(Deserialize)]
struct Held {
    #[serde(default)]
    speedlimit_abs: String,
    #[serde(default)]
    kbpersec: String,
}

#[async_trait]
impl Throttling for Sabnzbd {
    async fn throttled(&self) -> Result<Throttled, Failure> {
        let held: Limited = self
            .read("queue", "the rate limit could not be read")
            .await?;
        let down = absolute(&held.queue.speedlimit_abs);
        Ok(Throttled {
            rates: Rates { down, up: None },
            uploads: false,
            // Which side of the day it is on is read off its own schedule and its
            // own limit, both as they stand this moment, rather than worked out
            // from a clock this product does not have.
            hours: side(&self.schedule_lines().await?, down.is_some()),
        })
    }

    async fn restrain(&self, wanted: &Wanted) -> Result<Throttled, Failure> {
        restrain(self, wanted).await
    }

    async fn moving(&self) -> Result<Rates, Failure> {
        let held: Limited = self
            .read("queue", "the current transfer rate could not be read")
            .await?;
        Ok(Rates {
            down: bytes_per_second(&held.queue.kbpersec),
            up: None,
        })
    }
}

impl Sabnzbd {
    /// Write the standing rate limit, for a client with no window to switch on.
    async fn set_rate(&self, value: &str) -> Result<(), Failure> {
        let wrote: Wrote = self
            .read(
                &format!("config&name=speedlimit&value={value}"),
                "the rate limit could not be set",
            )
            .await?;
        if wrote.status {
            return Ok(());
        }
        Err(self
            .endpoint
            .refused("the client answered that it did not take the rate limit"))
    }
}

/// The schedule lines the household's day comes to, or none at all.
///
/// None wherever there is nothing for a schedule to do: no window declared, or a
/// window whose two sides come to the same limit. A pair of instructions that set
/// one figure twice is a schedule that switches nothing while looking like a
/// household's day, and it would report the client as keeping hours it does not.
fn turns(wanted: &Wanted) -> Vec<Turn> {
    let Some(window) = wanted.window else {
        return Vec::new();
    };
    let (active, quiet) = (kilobytes(wanted.active.down), kilobytes(wanted.quiet.down));
    if active == quiet {
        return Vec::new();
    }
    vec![
        Turn {
            hour: window.from_hour,
            minute: window.from_minute,
            figure: active,
        },
        Turn {
            hour: window.to_hour,
            minute: window.to_minute,
            figure: quiet,
        },
    ]
}

/// The client's `speedlimit_abs` as a limit, where it is one.
///
/// Empty or zero is nothing holding it back, which is not a limit of nothing — a
/// client told to move zero bytes a second would be a stopped client.
fn absolute(figure: &str) -> Option<u64> {
    figure.trim().parse::<u64>().ok().filter(|held| *held > 0)
}

/// A limit as the value `SABnzbd`'s own settings take.
///
/// Written with its unit rather than as a bare number, which the client would read
/// as a percentage where the operator has told it what their line carries — the
/// one reading that would turn a two-megabyte limit into two per cent of the line.
/// Rounded up, so a limit under a kilobyte becomes the smallest the client can
/// hold rather than none at all. Lower case, because a schedule line is stored in
/// the case the client puts it in and a figure written any other way never matches
/// what comes back.
fn kilobytes(limit: Option<u64>) -> String {
    limit.map_or_else(
        || "0".to_owned(),
        |bytes| format!("{}k", bytes.div_ceil(1024).max(1)),
    )
}

async fn restrain(sabnzbd: &Sabnzbd, wanted: &Wanted) -> Result<Throttled, Failure> {
    let turns = turns(wanted);
    let scheduled = !turns.is_empty();
    sabnzbd.keeping(&turns).await?;
    if !scheduled {
        // Nothing switches the rate, so the rate is this client's standing one.
        sabnzbd
            .set_rate(kilobytes(wanted.active.down).as_str())
            .await?;
    }

    // Read back rather than trusting that answer, the same as every other
    // write here: a client that took the request and did not apply it looks
    // like one that did, from out here.
    sabnzbd.throttled().await
}

#[cfg(test)]
mod tests;
