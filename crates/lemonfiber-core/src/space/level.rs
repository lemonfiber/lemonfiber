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
pub(crate) const EXHAUSTED_FLOOR: u64 = 256 * 1024 * 1024;

/// The room left at which nothing more can be relied on to fit.
///
/// A single film at the presets this product offers is a few gigabytes, so a
/// volume under this cannot take one more of anything the stack is likely to be
/// fetching.
pub(crate) const CRITICAL_FLOOR: u64 = 2 * 1024 * 1024 * 1024;

/// The share of a volume's own size below which it is worth mentioning.
///
/// Relative rather than absolute, because the absolute floors below it already
/// answer "is there room for the next thing" — this answers a different question,
/// which is whether the trend is worth knowing about before it is urgent, and a
/// tenth of a volume is small on any volume.
const COMFORTABLE_SHARE: u64 = 10;

/// Where a volume stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
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
    pub(crate) const fn halts(self) -> bool {
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
mod tests;
