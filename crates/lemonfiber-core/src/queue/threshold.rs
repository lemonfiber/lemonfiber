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
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::Thresholds;

    #[test]
    fn the_defaults_are_hours_and_days_rather_than_minutes() {
        // A check that cries stuck at the first slow hour is one the operator turns
        // off, taking everything else on the channel with it.
        let t = Thresholds::conservative();
        assert!(t.stalled >= Duration::from_secs(60 * 60), "{:?}", t.stalled);
        assert!(
            t.waiting >= Duration::from_secs(24 * 60 * 60),
            "{:?}",
            t.waiting
        );
    }

    #[test]
    fn something_finished_and_unimported_is_chased_soonest() {
        // Nothing about it recovers with time, unlike a torrent that may find a
        // seeder overnight — so it is the one threshold that is deliberately short.
        let t = Thresholds::conservative();
        assert!(t.not_imported < t.stalled);
        assert!(t.not_imported < t.waiting);
    }

    #[test]
    fn slow_is_noticed_before_stalled_because_it_is_only_a_note() {
        let t = Thresholds::conservative();
        assert!(t.slow < t.stalled);
    }

    #[test]
    fn a_watchlist_is_given_the_longest_rope() {
        // A film that is not out yet is waiting for a reason; impatience here would
        // flag every unreleased thing the household asked for.
        let t = Thresholds::conservative();
        let longest = [t.stalled, t.slow, t.not_imported, t.waiting]
            .into_iter()
            .max();
        assert_eq!(longest, Some(t.waiting));
    }

    #[test]
    fn a_threshold_not_yet_reached_says_nothing() {
        let t = Thresholds::conservative();
        let a_moment = Duration::from_secs(1);
        assert!(t.within(t.stalled, t.stalled.saturating_sub(a_moment)));
        assert!(
            !t.within(t.stalled, t.stalled),
            "the line itself is reached"
        );
        assert!(!t.within(t.stalled, t.stalled.saturating_add(a_moment)));
    }

    #[test]
    fn the_thresholds_can_be_moved_off_the_defaults() {
        // A fast connection with a good indexer and rural broadband do not agree
        // about what an hour means.
        let impatient = Thresholds {
            stalled: Duration::from_secs(60),
            ..Thresholds::conservative()
        };
        assert_ne!(impatient, Thresholds::default());
        assert!(!impatient.within(impatient.stalled, Duration::from_secs(120)));
    }
}
