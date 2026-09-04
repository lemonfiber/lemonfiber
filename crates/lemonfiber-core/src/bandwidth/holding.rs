//! Whether a limit took, and whether it is holding.
//!
//! A cap the operator cannot see the effect of is a cap they turn off. So a limit
//! is not called applied because a client answered `200`: it is read back, and then
//! the throughput is read beside it.
//!
//! Those are two different failures and they are kept apart. A client that reports
//! a figure other than the one it was given did not take the setting; a client
//! that reports the right figure and moves faster than it took the setting and is
//! not honouring it. The first is fixed by looking at the client's configuration
//! and the second is not, so telling an operator "the limit is not working" for
//! both would send them to the wrong place half the time.

use serde::Serialize;

use super::rhythm::Period;

/// How far over a limit a client may read before it is called an overrun, in whole
/// per cent.
///
/// A rate is an average over whatever window the client averages over, and the
/// figure it reports bounces around the limit rather than sitting under it. Calling
/// every bounce an overrun would put a warning on a perfectly obedient client,
/// which is how a report stops being read.
pub const TOLERANCE: u64 = 10;

/// What became of one limit, in one direction, on one client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Nothing was asked for, so there is nothing to hold to.
    Unasked,
    /// This client has no such direction to limit at all.
    ///
    /// Apart from a limit that was ignored, and deliberately: a Usenet client does
    /// not upload, so an upload limit on one is not a setting it refused.
    NothingToLimit,
    /// It took the limit and is inside it.
    Holding,
    /// It reports a figure other than the one it was given.
    Ignored,
    /// It reports the right figure and is moving faster than it.
    Overrunning,
}

impl Verdict {
    /// Whether this is something the operator needs to be told.
    #[must_use]
    pub const fn worth_saying(self) -> bool {
        matches!(self, Self::Ignored | Self::Overrunning)
    }

    /// What it means, in the words it is shown in.
    #[must_use]
    pub const fn means(self) -> &'static str {
        match self {
            Self::Unasked => "nothing was asked of it in this direction",
            Self::NothingToLimit => {
                "this client has nothing to limit in this direction — Usenet does not upload"
            }
            Self::Holding => "the limit was accepted and is being kept to",
            Self::Ignored => {
                "the client reports a different limit than it was given, so the setting did \
                 not take — look at the client's own configuration"
            }
            Self::Overrunning => {
                "the client reports the limit and is moving faster than it, so the setting \
                 took and is not being honoured"
            }
        }
    }
}

/// One direction on one client: what it was asked for, took, and is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Held {
    /// What it was asked to hold to, in bytes a second, where anything was.
    pub asked: Option<u64>,
    /// What it reports as in force.
    pub accepted: Option<u64>,
    /// What it is moving right now, where it reported a figure.
    pub moving: Option<u64>,
    /// What that adds up to.
    pub verdict: Verdict,
}

impl Held {
    /// Judge one direction from what was asked, what came back, and what is moving.
    ///
    /// `exists` says whether this client has this direction at all, which is the
    /// one thing none of the three figures could tell apart from a limit ignored.
    #[must_use]
    pub fn of(
        asked: Option<u64>,
        accepted: Option<u64>,
        moving: Option<u64>,
        exists: bool,
    ) -> Self {
        let verdict = if !exists {
            Verdict::NothingToLimit
        } else if asked.is_none() {
            Verdict::Unasked
        } else if asked != accepted {
            Verdict::Ignored
        } else if over(accepted, moving) {
            Verdict::Overrunning
        } else {
            Verdict::Holding
        };
        Self {
            asked,
            accepted,
            moving,
            verdict,
        }
    }
}

/// Whether what is moving is far enough past the limit to be worth calling out.
fn over(limit: Option<u64>, moving: Option<u64>) -> bool {
    let (Some(limit), Some(moving)) = (limit, moving) else {
        return false;
    };
    // Scaled before dividing so a small limit keeps its margin, and saturating so
    // a limit near the top of the range does not wrap into a tiny one.
    moving > limit.saturating_add(limit.saturating_mul(TOLERANCE) / 100)
}

/// How one download client answered about the limits on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case", tag = "answered")]
pub enum Answer {
    /// It answered, in both directions.
    Held {
        /// What became of the download limit.
        down: Held,
        /// And of the upload one.
        up: Held,
        /// Which side of the household's day it says it is on, where it keeps the
        /// hours itself.
        period: Option<Period>,
    },
    /// It did not answer, and this is what it said.
    ///
    /// Its own line rather than an absence, because a client nobody could reach is
    /// a client whose limits are unknown, and an unknown limit rendered as no
    /// limit is the report reading better than the stack is.
    Silent {
        /// What went wrong, in the words of whatever refused.
        said: String,
    },
}

/// One download client, and what became of the limits it was given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Holding {
    /// The client, by the name the stack knows it under.
    pub client: String,
    /// What it said.
    pub answer: Answer,
}

impl Holding {
    /// Whether anything about this client is worth putting in front of an operator.
    #[must_use]
    pub fn worth_saying(&self) -> bool {
        match &self.answer {
            Answer::Silent { .. } => true,
            Answer::Held { down, up, .. } => {
                down.verdict.worth_saying() || up.verdict.worth_saying()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Answer, Held, Holding, Verdict, TOLERANCE};
    use crate::bandwidth::rhythm::Period;

    /// A megabyte a second, which every case here is measured against.
    const LIMIT: u64 = 1024 * 1024;

    #[test]
    fn a_client_that_took_the_limit_and_is_inside_it_is_holding() {
        let held = Held::of(Some(LIMIT), Some(LIMIT), Some(LIMIT / 2), true);
        assert_eq!(held.verdict, Verdict::Holding);
        assert!(!held.verdict.worth_saying());
    }

    #[test]
    fn a_client_reporting_a_different_figure_did_not_take_the_setting() {
        // The operator is sent to the client's own configuration, which is where
        // this one is fixed.
        let held = Held::of(Some(LIMIT), Some(LIMIT * 4), Some(0), true);
        assert_eq!(held.verdict, Verdict::Ignored);
        assert!(held.verdict.means().contains("own configuration"));
        assert!(held.verdict.worth_saying());
    }

    #[test]
    fn a_client_reporting_no_limit_at_all_did_not_take_it_either() {
        let held = Held::of(Some(LIMIT), None, Some(0), true);
        assert_eq!(held.verdict, Verdict::Ignored);
    }

    #[test]
    fn a_client_moving_past_the_limit_it_holds_is_not_honouring_it() {
        // A different failure from the one above, and it sends the operator
        // somewhere else, so the two are never rendered alike.
        let past = LIMIT + LIMIT * TOLERANCE / 100 + 1;
        let held = Held::of(Some(LIMIT), Some(LIMIT), Some(past), true);
        assert_eq!(held.verdict, Verdict::Overrunning);
        assert!(held.verdict.means().contains("not being honoured"));
    }

    #[test]
    fn a_rate_bouncing_around_the_limit_is_not_an_overrun() {
        // A rate is an average over the client's own window, and it sits either
        // side of the figure rather than under it. Calling every bounce an
        // overrun puts a warning on an obedient client, which is how a report
        // stops being read.
        let inside = LIMIT + LIMIT * TOLERANCE / 100;
        assert_eq!(
            Held::of(Some(LIMIT), Some(LIMIT), Some(inside), true).verdict,
            Verdict::Holding
        );
    }

    #[test]
    fn a_limit_near_the_top_of_the_range_does_not_wrap_into_a_tiny_one() {
        assert_eq!(
            Held::of(Some(u64::MAX), Some(u64::MAX), Some(u64::MAX), true).verdict,
            Verdict::Holding
        );
    }

    #[test]
    fn a_direction_this_client_does_not_have_is_not_a_limit_it_ignored() {
        // Usenet does not upload. Reporting that as a refused setting would send
        // an operator looking for a fault in a client that is behaving perfectly.
        let held = Held::of(Some(LIMIT), None, None, false);
        assert_eq!(held.verdict, Verdict::NothingToLimit);
        assert!(!held.verdict.worth_saying());
        assert!(held.verdict.means().contains("does not upload"));
    }

    #[test]
    fn a_direction_nothing_was_asked_of_is_neither_holding_nor_failing() {
        let held = Held::of(None, None, Some(LIMIT), true);
        assert_eq!(held.verdict, Verdict::Unasked);
        assert!(!held.verdict.worth_saying());
        assert!(held.verdict.means().contains("nothing was asked"));
    }

    #[test]
    fn a_client_that_could_not_be_read_is_always_worth_saying() {
        // An unknown limit rendered as no limit is a report reading better than
        // the stack is.
        let silent = Holding {
            client: "qbittorrent".to_owned(),
            answer: Answer::Silent {
                said: "connection refused".to_owned(),
            },
        };
        assert!(silent.worth_saying());
    }

    #[test]
    fn a_client_holding_in_both_directions_is_not_worth_interrupting_anybody_for() {
        let quiet = Holding {
            client: "qbittorrent".to_owned(),
            answer: Answer::Held {
                down: Held::of(Some(LIMIT), Some(LIMIT), Some(0), true),
                up: Held::of(Some(LIMIT), Some(LIMIT), Some(0), true),
                period: Some(Period::Quiet),
            },
        };
        assert!(!quiet.worth_saying());
    }
}
