//! What the line was measured to carry, and how much that measurement is worth.
//!
//! A proportion is only as good as the figure it is a proportion of, so the
//! measurement is a first-class thing here rather than a number tucked inside a
//! setting: where it came from, when it was taken, and whether it is old enough to
//! stop trusting.
//!
//! **Nothing here runs a speed test.** The figure is either one the operator
//! declared — they have a bill with a number on it — or the fastest the stack has
//! actually been seen to move, kept as a high-water mark and raised whenever a run
//! sees better. That second reading costs no traffic, disturbs nobody else on the
//! line, and has the property a speed test does not: it is what this stack, on this
//! machine, through whatever path its downloads take, really achieved. Where that
//! path is a VPN tunnel the figure is the tunnel's throughput and not the line's,
//! which is lower — and is said, because a limit set as a share of a tunnel figure
//! is a smaller limit than the operator thinks they asked for.

use serde::{Deserialize, Serialize};

/// How long a measurement stands before it is worth taking again, in seconds.
///
/// A month. Lines change — a provider upgrade, a neighbour, a different router —
/// and a share pinned to a reading from last spring is a share of a number that no
/// longer exists. Long enough that an idle stack is not nagged, short enough that
/// nobody lives a year on one reading.
pub(crate) const GOES_STALE_AFTER: u64 = 30 * 24 * 60 * 60;

/// Where a figure for the line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// The operator gave it, presumably off the plan they pay for.
    Declared,
    /// The fastest the stack has been seen to move, which is what it achieved
    /// rather than what the line is sold as.
    Observed,
}

impl Source {
    /// What this figure is, in the words it is shown in.
    #[must_use]
    pub const fn means(self) -> &'static str {
        match self {
            Self::Declared => "what you told lemonfiber this line carries",
            Self::Observed => {
                "the fastest the stack has been seen to move, which is \
                              what it achieved rather than what the line is sold as"
            }
        }
    }
}

/// How much a measurement can be relied on now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case", tag = "reading", content = "days")]
#[schemars(rename = "CapacityStanding")]
pub enum Standing {
    /// Recent enough to set a share against.
    Fresh,
    /// Old enough that the line may have changed under it, with how many days it
    /// has stood.
    Stale(u64),
}

/// What the line was measured to carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Capacity {
    /// Bytes a second down.
    pub down: u64,
    /// Bytes a second up.
    ///
    /// Measured apart from the download, because a home connection is asymmetric
    /// and a single figure for both would make every upload share far larger than
    /// the operator meant.
    pub up: u64,
    /// Where the figure came from.
    pub source: Source,
    /// When it was taken, in seconds since the epoch.
    pub taken: u64,
    /// Whether the path it was measured over goes through the VPN tunnel.
    pub through_tunnel: bool,
}

impl Capacity {
    /// How this reading stands against the clock.
    #[must_use]
    pub const fn standing(&self, now: u64) -> Standing {
        let age = now.saturating_sub(self.taken);
        if age < GOES_STALE_AFTER {
            Standing::Fresh
        } else {
            Standing::Stale(age / (24 * 60 * 60))
        }
    }

    /// What is worth saying about this reading beside the figures themselves.
    ///
    /// Only what bears on trusting it. A fresh reading off a declared figure adds
    /// nothing to what the numbers already say, so it says nothing rather than
    /// filling the report with a sentence that is true of every stack.
    #[must_use]
    pub fn cautions(&self, now: u64) -> Vec<String> {
        let mut said = Vec::new();
        if let Standing::Stale(days) = self.standing(now) {
            said.push(format!(
                "This figure has stood for {days} days. A line changes, and a share \
                 of a number that no longer exists is not the limit you set."
            ));
        }
        if self.through_tunnel {
            said.push(
                "It was measured through the VPN tunnel, which carries less than the \
                 line beneath it — so a share taken from it is a share of the tunnel."
                    .to_owned(),
            );
        }
        said
    }

    /// The two figures raised to whatever a fresh reading saw, where it saw better.
    ///
    /// Raised rather than replaced, and only for an observed figure: a line's
    /// capacity is what it has been seen to do at its best, and a reading taken
    /// while the stack was idle, or while somebody was on a video call, says
    /// nothing about the line at all. A figure the operator declared is theirs and
    /// is never overwritten by an observation.
    #[must_use]
    pub(crate) fn raised_by(self, seen: Self) -> Self {
        if self.source == Source::Declared {
            return self;
        }
        Self {
            down: self.down.max(seen.down),
            up: self.up.max(seen.up),
            source: Source::Observed,
            taken: self.taken.max(seen.taken),
            through_tunnel: self.through_tunnel || seen.through_tunnel,
        }
    }
}

#[cfg(test)]
mod tests;
