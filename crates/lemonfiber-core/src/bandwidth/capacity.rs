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
pub const GOES_STALE_AFTER: u64 = 30 * 24 * 60 * 60;

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
    pub fn raised_by(self, seen: Self) -> Self {
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
mod tests {
    use super::{Capacity, Source, Standing, GOES_STALE_AFTER};

    /// A moment every case here reads against.
    const NOW: u64 = 1_790_812_800;

    /// A reading of a ten-megabyte line, taken now.
    fn measured(source: Source) -> Capacity {
        Capacity {
            down: 10 * 1024 * 1024,
            up: 1024 * 1024,
            source,
            taken: NOW,
            through_tunnel: false,
        }
    }

    #[test]
    fn a_reading_taken_today_is_one_to_set_a_share_against() {
        assert_eq!(measured(Source::Observed).standing(NOW), Standing::Fresh);
        assert!(measured(Source::Observed).cautions(NOW).is_empty());
    }

    #[test]
    fn a_reading_that_has_stood_too_long_says_how_long() {
        let old = Capacity {
            taken: NOW - GOES_STALE_AFTER - 24 * 60 * 60,
            ..measured(Source::Observed)
        };
        assert_eq!(old.standing(NOW), Standing::Stale(31));
        let said = old.cautions(NOW);
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(
            said.first().is_some_and(|line| line.contains("31 days")),
            "{said:?}"
        );
    }

    #[test]
    fn a_reading_taken_through_the_tunnel_says_it_is_the_tunnels() {
        // A share of the tunnel is a smaller limit than the operator asked for,
        // and finding that out from the throughput is finding it out too late.
        let tunnelled = Capacity {
            through_tunnel: true,
            ..measured(Source::Observed)
        };
        let said = tunnelled.cautions(NOW);
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(
            said.first().is_some_and(|line| line.contains("tunnel")),
            "{said:?}"
        );
    }

    #[test]
    fn a_clock_that_reads_before_the_measurement_does_not_age_it() {
        // Neither a fault nor an ancient reading: a machine whose clock went
        // backwards has a measurement from its own future, and calling that
        // stale would be reporting the clock as a network problem.
        assert_eq!(measured(Source::Observed).standing(0), Standing::Fresh);
    }

    #[test]
    fn an_observed_figure_is_raised_by_a_better_one_and_never_lowered() {
        let seen = Capacity {
            down: 20 * 1024 * 1024,
            up: 512 * 1024,
            taken: NOW + 60,
            ..measured(Source::Observed)
        };
        let raised = measured(Source::Observed).raised_by(seen);
        assert_eq!(raised.down, 20 * 1024 * 1024, "the better reading wins");
        assert_eq!(raised.up, 1024 * 1024, "and the worse one does not");
        assert_eq!(raised.taken, NOW + 60);
    }

    #[test]
    fn a_figure_the_operator_declared_is_never_overwritten_by_an_observation() {
        let seen = Capacity {
            down: 1,
            up: 1,
            source: Source::Observed,
            taken: NOW + 60,
            through_tunnel: true,
        };
        let kept = measured(Source::Declared).raised_by(seen);
        assert_eq!(kept, measured(Source::Declared));
    }

    #[test]
    fn where_a_figure_came_from_is_said_in_words() {
        assert!(Source::Declared.means().contains("you told"));
        assert!(Source::Observed.means().contains("achieved"));
    }
}
