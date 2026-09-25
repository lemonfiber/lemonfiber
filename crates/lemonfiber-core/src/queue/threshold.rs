//! How long something has to have been wrong before anyone is told.
//!
//! "Stuck" is a judgement, not a measurement. A torrent with no seeders may
//! recover in a day; one that has not moved in a week will not. So the line is
//! time, and the defaults sit deliberately far out: a queue check that cries stuck
//! at the first slow hour teaches the operator to ignore it, and an ignored check
//! is worse than an absent one because it also carries the leak alert.
//!
//! Adjustable, because a machine on a fast connection with a good indexer and one
//! on rural broadband do not agree about what an hour means.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::Stall;

/// How long each kind of stillness has to last before it is worth saying.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thresholds {
    /// No progress at all. Long, because a torrent finding a seeder overnight is
    /// ordinary and reporting it at hour one would be wrong more often than right.
    pub stalled: Duration,
    /// Moving, but slowly. Shorter than a stall, because it is only a note.
    pub slow: Duration,
    /// Finished downloading and still not in the library. Short: nothing about
    /// this recovers with time, and every hour of it is an hour the operator
    /// thinks something is coming that is not.
    pub not_imported: Duration,
    /// Monitored and never grabbed. Longest of all — a film that is not out yet
    /// is waiting for a reason, and impatience here would flag the whole
    /// watchlist.
    pub waiting: Duration,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self::conservative()
    }
}

impl Thresholds {
    /// The defaults, chosen to be wrong in the direction of saying nothing.
    #[must_use]
    pub const fn conservative() -> Self {
        Self {
            stalled: Duration::from_secs(6 * 60 * 60),
            slow: Duration::from_secs(2 * 60 * 60),
            not_imported: Duration::from_secs(60 * 60),
            waiting: Duration::from_secs(14 * 24 * 60 * 60),
        }
    }

    /// Whether something still has time before it is worth reporting.
    #[must_use]
    pub fn within(self, kind: Duration, held: Duration) -> bool {
        held < kind
    }

    /// How long this kind of stall has to last before it is worth saying.
    ///
    /// A loop and a repeated import failure are structural — they will not resolve
    /// themselves, and waiting only spends more of the allowance — so they are
    /// said as soon as they are seen. Everything else waits.
    #[must_use]
    pub(crate) const fn for_stall(self, stall: Stall) -> Duration {
        match stall {
            Stall::RedownloadLoop | Stall::RepeatedImportFailure => Duration::ZERO,
            Stall::CompletedNotImported | Stall::Orphaned => self.not_imported,
            Stall::StalledDownload => self.stalled,
            Stall::WaitingIndefinitely => self.waiting,
            Stall::Slow => self.slow,
        }
    }
}

#[cfg(test)]
mod tests;
