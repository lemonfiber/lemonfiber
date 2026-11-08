//! Why a download is unhealthy, and when it has been unhealthy long enough to act on.
//!
//! An item is judged from what its curator says of it and what its download client says,
//! paired by the client's own id. Nothing here asks either: what they answered is handed
//! in, and what it comes to is handed back.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::instant::DAY;
use crate::model::Wedge;
use crate::ports::service::{Download, DownloadState, Downloading, TransferState};

/// Why `entry`, the curator's view, and `item`, the client's, make a download unhealthy,
/// in words; nothing where it is healthy or there is nothing to judge.
///
/// An item a client holds and no curator does is not judged here: whether it is an orphan
/// turns on the category it is under, which the caller holds.
#[must_use]
pub fn classify(
    entry: Option<&Downloading>,
    item: Option<&Download>,
    slow: Duration,
    now: u64,
) -> Option<(Wedge, String)> {
    let entry = entry?;
    let state = item.and_then(|item| item.state);
    if entry.state == DownloadState::Failed {
        return Some((Wedge::Failed, "its curator gave up on it".to_owned()));
    }
    match state {
        Some(TransferState::Error) => {
            return Some((
                Wedge::Failed,
                "its client reports an error it will not recover from".to_owned(),
            ))
        }
        Some(TransferState::Metadata) => {
            return Some((
                Wedge::StuckOnMetadata,
                "the torrent has not resolved what it is".to_owned(),
            ))
        }
        Some(TransferState::Stalled) => {
            return Some((
                Wedge::Stalled,
                "it is connected with nothing moving and no source to move it".to_owned(),
            ))
        }
        Some(TransferState::Downloading | TransferState::Queued | TransferState::Paused) | None => {
        }
    }
    if state.is_none() && entry.state == DownloadState::Warning {
        return Some((
            Wedge::Stalled,
            "its curator tracks it as not progressing".to_owned(),
        ));
    }
    let left = item
        .and_then(|item| item.eta)
        .or_else(|| {
            entry
                .expected
                .and_then(|expected| u64::try_from(expected).ok())
                .map(|expected| Duration::from_secs(expected.saturating_sub(now)))
        })
        .filter(|left| *left > slow)?;
    Some((
        Wedge::Slow,
        format!(
            "at its speed it would take {} more days, past the {} allowed",
            left.as_secs() / DAY,
            slow.as_secs() / DAY
        ),
    ))
}

/// Why `item`, which no curator's queue holds, is an orphan: it is under `category`, one
/// of a curator's.
#[must_use]
pub fn orphaned(category: &str) -> (Wedge, String) {
    (
        Wedge::Orphaned,
        format!("its client holds it under {category}, and no curator's queue does"),
    )
}

/// An item's strikes: the reason it was last flagged for, how many consecutive passes
/// flagged it for that reason, and when it was first flagged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Struck {
    /// The reason.
    pub wedge: Wedge,
    /// How many consecutive passes.
    pub strikes: u32,
    /// When it was first flagged, in seconds since the Unix epoch.
    pub since: u64,
}

/// An item's strikes after a pass that found it unhealthy for `wedge`: one more for the
/// same reason, and one for a new reason.
#[must_use]
pub fn strike(before: Option<Struck>, wedge: Wedge, now: u64) -> Struck {
    match before {
        Some(before) if before.wedge == wedge => Struck {
            strikes: before.strikes.saturating_add(1),
            ..before
        },
        Some(_) | None => Struck {
            wedge,
            strikes: 1,
            since: now,
        },
    }
}

/// Until when an item added at `added` is left alone, where it still is at `now`.
#[must_use]
pub fn in_grace_until(added: u64, grace: Duration, now: u64) -> Option<u64> {
    let until = added.saturating_add(grace.as_secs());
    (until > now).then_some(until)
}

#[cfg(test)]
mod tests;
