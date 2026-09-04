//! How full is too full, and what each answer costs.
//!
//! Five steps rather than a threshold, because "the disk is nearly full" is not
//! one condition: room enough to mention is not room enough to warn about, and
//! neither is the point at which a service database can no longer write. Bundling
//! them into one alert means either shouting early — which is how an operator
//! learns to ignore the alert — or shouting late, which is how a database is
//! corrupted.
//!
//! The last step is the only one that acts on its own. Everything below it is said
//! and left with the operator; at the top, new work is stopped, because a database
//! that cannot write may not merely fail — it may take its file with it, and a
//! space problem that became a data-loss problem is not one an apology fixes.

use serde::Serialize;

use crate::doctor::storage::LOW_SPACE_FLOOR;

/// The room a volume must keep clear for the services running on it to keep
/// working at all.
///
/// Not room to download into — room for a database to write a transaction, a log
/// to be appended to, a temporary file to be made while an import runs. Below this
/// the stack is not short of space, it is failing, and every service on the volume
/// is failing at once.
pub const EXHAUSTED_FLOOR: u64 = 256 * 1024 * 1024;

/// The room left at which nothing more can be relied on to fit.
///
/// A single film at the presets this product offers is a few gigabytes, so a
/// volume under this cannot take one more of anything the stack is likely to be
/// fetching.
pub const CRITICAL_FLOOR: u64 = 2 * 1024 * 1024 * 1024;

/// The share of a volume's own size below which it is worth mentioning.
///
/// Relative rather than absolute, because the absolute floors below it already
/// answer "is there room for the next thing" — this answers a different question,
/// which is whether the trend is worth knowing about before it is urgent, and a
/// tenth of a volume is small on any volume.
const COMFORTABLE_SHARE: u64 = 10;

/// Where a volume stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// Usage cannot be determined.
    ///
    /// Ordered first so that it never wins a comparison against a level that was
    /// actually measured: a volume nobody could read must not make a stack that is
    /// visibly full read as merely unknown.
    Unknown,
    /// Comfortable headroom.
    Ample,
    /// Below comfortable, not urgent.
    Advisory,
    /// Projected to exhaust within the horizon.
    Warning,
    /// Nearly exhausted.
    Critical,
    /// Full; acquisitions halted.
    Exhausted,
}

impl Level {
    /// Where a volume stands, from what is free now and what is already committed
    /// to landing on it.
    ///
    /// The middle three steps are read off the *projected* figure rather than the
    /// free one, which is the whole point: a volume with forty gigabytes free and
    /// sixty gigabytes of queue is going to fill, and saying so once it has filled
    /// is a report rather than a warning.
    ///
    /// The top step is read off what is free now, because it is not a prediction.
    /// A database cannot write into space the queue has not consumed yet, and
    /// halting acquisitions on a volume that is merely going to be full would stop
    /// a working stack.
    #[must_use]
    pub fn reached(free: Option<u64>, limit: Option<u64>, committed: u64) -> Self {
        let (Some(free), Some(limit)) = (free, limit) else {
            return Self::Unknown;
        };
        if limit == 0 {
            return Self::Unknown;
        }
        if free < EXHAUSTED_FLOOR {
            return Self::Exhausted;
        }
        let projected = free.saturating_sub(committed);
        if projected < CRITICAL_FLOOR {
            return Self::Critical;
        }
        if projected < LOW_SPACE_FLOOR {
            return Self::Warning;
        }
        if free < limit / COMFORTABLE_SHARE {
            return Self::Advisory;
        }
        Self::Ample
    }

    /// Whether reaching this level stops new work being started.
    ///
    /// One level does, and it is the one where carrying on risks more than the
    /// work being stopped would.
    #[must_use]
    pub const fn halts(self) -> bool {
        matches!(self, Self::Exhausted)
    }

    /// Whether this level is worth saying anything about at all.
    #[must_use]
    pub const fn worth_saying(self) -> bool {
        !matches!(self, Self::Ample | Self::Unknown)
    }

    /// The level as it is written and read back.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Ample => "ample",
            Self::Advisory => "advisory",
            Self::Warning => "warning",
            Self::Critical => "critical",
            Self::Exhausted => "exhausted",
        }
    }

    /// What this level means for the operator, in one sentence.
    #[must_use]
    pub const fn means(self) -> &'static str {
        match self {
            Self::Unknown => "how much room is left could not be established",
            Self::Ample => "there is comfortable room",
            Self::Advisory => "there is less room than is comfortable, and nothing urgent",
            Self::Warning => "what is already queued will not fit alongside what is here",
            Self::Critical => "the next thing to land will not fit",
            Self::Exhausted => {
                "there is no room left to write in, so new acquisitions are halted to keep the \
                 services' own databases writable"
            }
        }
    }

    /// The worst of a set of levels, or unknown where there were none.
    ///
    /// A stack is as well off as its worst volume: a data location with room to
    /// spare is no comfort when the volume the service configuration sits on is
    /// full, since either filling stops the same stack.
    #[must_use]
    pub fn worst(levels: impl IntoIterator<Item = Self>) -> Self {
        levels.into_iter().max().unwrap_or(Self::Unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::{Level, CRITICAL_FLOOR, EXHAUSTED_FLOOR};
    use crate::doctor::storage::LOW_SPACE_FLOOR;

    /// A volume of a terabyte, which is large enough that the relative step never
    /// fires by accident in the cases about the absolute ones.
    const LARGE: u64 = 1024 * 1024 * 1024 * 1024;

    #[test]
    fn a_volume_nobody_could_read_is_unknown_rather_than_full() {
        assert_eq!(Level::reached(None, Some(LARGE), 0), Level::Unknown);
        assert_eq!(Level::reached(Some(500), None, 0), Level::Unknown);
        assert_eq!(
            Level::reached(Some(500), Some(0), 0),
            Level::Unknown,
            "a volume reporting no size at all was not measured"
        );
        assert!(!Level::Unknown.worth_saying());
        assert!(!Level::Unknown.halts());
    }

    #[test]
    fn a_volume_with_room_to_spare_is_ample() {
        let level = Level::reached(Some(LARGE / 2), Some(LARGE), 0);
        assert_eq!(level, Level::Ample);
        assert!(
            !level.worth_saying(),
            "nothing to say about a healthy volume"
        );
    }

    #[test]
    fn exhaustion_is_projected_from_what_is_already_committed() {
        // The requirement this whole module exists for: forty gigabytes free and a
        // queue that will take thirty-five of them is a volume about to fill, and
        // the free figure on its own says it is fine.
        let volume = 100 * 1024 * 1024 * 1024;
        let free = 40 * 1024 * 1024 * 1024;
        let committed = 35 * 1024 * 1024 * 1024;
        assert_eq!(
            Level::reached(Some(free), Some(volume), 0),
            Level::Ample,
            "nothing committed, so the same volume is comfortable"
        );
        assert_eq!(
            Level::reached(Some(free), Some(volume), committed),
            Level::Warning
        );
    }

    #[test]
    fn the_steps_escalate_in_order_as_the_projection_falls() {
        let ample = Level::reached(Some(LARGE / 2), Some(LARGE), 0);
        let advisory = Level::reached(Some(LARGE / 100), Some(LARGE), 0);
        let warning = Level::reached(
            Some(LOW_SPACE_FLOOR + CRITICAL_FLOOR),
            Some(LARGE),
            CRITICAL_FLOOR + 1,
        );
        let critical = Level::reached(Some(CRITICAL_FLOOR + 1), Some(LARGE), 2);
        let exhausted = Level::reached(Some(EXHAUSTED_FLOOR - 1), Some(LARGE), 0);
        assert_eq!(
            [ample, advisory, warning, critical, exhausted],
            [
                Level::Ample,
                Level::Advisory,
                Level::Warning,
                Level::Critical,
                Level::Exhausted
            ]
        );
        assert!(
            ample < advisory && advisory < warning && warning < critical && critical < exhausted
        );
    }

    #[test]
    fn a_volume_below_comfortable_is_advisory_on_its_own_size() {
        // A twentieth of a terabyte is fifty gigabytes, which passes every absolute
        // floor and is still a volume worth mentioning before it becomes urgent.
        let level = Level::reached(Some(LARGE / 20), Some(LARGE), 0);
        assert_eq!(level, Level::Advisory);
        assert!(level.worth_saying());
        assert!(!level.halts());
    }

    #[test]
    fn only_exhaustion_halts_new_work() {
        for level in [
            Level::Unknown,
            Level::Ample,
            Level::Advisory,
            Level::Warning,
            Level::Critical,
        ] {
            assert!(!level.halts(), "{} does not halt", level.word());
        }
        assert!(Level::Exhausted.halts());
        let means = Level::Exhausted.means();
        assert!(
            means.contains("databases"),
            "the halt says what it is protecting: {means}"
        );
    }

    #[test]
    fn exhaustion_is_read_from_what_is_free_rather_than_from_the_projection() {
        // A volume with room now and a queue that will consume all of it is going
        // to be full; halting a working stack on a prediction would stop work that
        // still fits.
        let free = 20 * 1024 * 1024 * 1024;
        assert_eq!(
            Level::reached(Some(free), Some(LARGE), free),
            Level::Critical,
            "projected to nothing, and still writable now"
        );
    }

    #[test]
    fn a_stack_is_as_well_off_as_its_worst_volume() {
        assert_eq!(
            Level::worst([Level::Ample, Level::Exhausted]),
            Level::Exhausted
        );
        assert_eq!(
            Level::worst([Level::Unknown, Level::Advisory]),
            Level::Advisory,
            "an unread volume never outranks a measured one"
        );
        assert_eq!(Level::worst([]), Level::Unknown);
    }

    #[test]
    fn every_level_reads_back_as_a_word_and_says_what_it_means() {
        let levels = [
            Level::Unknown,
            Level::Ample,
            Level::Advisory,
            Level::Warning,
            Level::Critical,
            Level::Exhausted,
        ];
        assert_eq!(levels.len(), 6, "every state the report can be in");
        for level in levels {
            let word = level.word();
            let means = level.means();
            assert!(!word.is_empty());
            assert!(means.len() > 20, "{word} says what it means: {means}");
        }
    }
}
