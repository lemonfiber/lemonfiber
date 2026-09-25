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
pub(crate) const TOLERANCE: u64 = 10;

/// What became of one limit, in one direction, on one client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "BandwidthVerdict")]
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
#[schemars(rename = "BandwidthHeld")]
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

/// Whether a client is fetching at all, where a cap made it a question.
///
/// Apart from the limits above rather than folded in with them, because it answers
/// a different question and a client held to a crawl is not a client that stopped.
/// A word that named both would be the vocabulary this whole path exists to avoid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Pulling {
    /// It is fetching, or would start on the next thing handed to it.
    Fetching,
    /// Nothing is moving and nothing new would start.
    Stopped,
}

impl Pulling {
    /// What this means for the household, in the words it is shown in.
    #[must_use]
    pub const fn means(self) -> &'static str {
        match self {
            Self::Fetching => "fetching",
            Self::Stopped => "stopped, and taking nothing new",
        }
    }
}

/// One download client, and what became of the limits it was given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Holding {
    /// The client, by the name the stack knows it under.
    pub client: String,
    /// What it said.
    pub answer: Answer,
    /// Whether it is fetching at all, where a declared cap made that a question.
    ///
    /// Absent on a stack with no cap rather than assumed to be fetching: asking
    /// every client whether it has stopped, on a stack where nothing would ever
    /// stop it, is traffic spent on a figure nothing would act on.
    pub pulling: Option<Pulling>,
}

impl Holding {
    /// Whether anything about this client is worth putting in front of an operator.
    #[must_use]
    pub fn worth_saying(&self) -> bool {
        if self.pulling == Some(Pulling::Stopped) {
            return true;
        }
        match &self.answer {
            Answer::Silent { .. } => true,
            Answer::Held { down, up, .. } => {
                down.verdict.worth_saying() || up.verdict.worth_saying()
            }
        }
    }
}

#[cfg(test)]
mod tests;
