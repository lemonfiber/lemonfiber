//! Lifting the limits for a while, and only for a while.
//!
//! There is always an evening when the operator wants something now and knows
//! nobody else is on the line. A limit with no way round it is a limit that gets
//! turned off permanently the first time it is inconvenient, so the way round it is
//! part of the design rather than an admission.
//!
//! It is time-boxed by construction: there is no way to ask for one that does not
//! end, and no way to ask for one that outlives an evening. Something switched off
//! "just for now" at eleven at night is the thing nobody remembers at eight the
//! next morning, and the household finds out during the school run.
//!
//! An expiry that has passed is reported before it is cleared, because a limit that
//! came back on its own is exactly the thing an operator wondering why the download
//! slowed down needs to be told.

use serde::{Deserialize, Serialize};

/// The longest a respite may run, in seconds.
///
/// Four hours: long enough for the evening it exists for, short enough that one
/// left running is over before anybody is inconvenienced by it.
pub const LONGEST: u64 = 4 * 60 * 60;

/// Limits lifted until a moment that is already fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Respite {
    /// When it stops, in seconds since the epoch.
    pub until: u64,
}

/// Where a respite stands against the clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case", tag = "standing", content = "seconds")]
#[schemars(rename = "RespiteStanding")]
pub enum Standing {
    /// None was asked for.
    None,
    /// In force, with this long left.
    InForce(u64),
    /// It ran out, this long ago. Said once, then cleared.
    Expired(u64),
}

impl Respite {
    /// A respite of `seconds` from now, or nothing where that is not a length one
    /// may ask for.
    ///
    /// The refusal is the requirement. A respite with no end, or one long enough to
    /// still be running tomorrow, is the setting that quietly becomes permanent.
    #[must_use]
    pub const fn asked_for(now: u64, seconds: u64) -> Option<Self> {
        if seconds == 0 || seconds > LONGEST {
            return None;
        }
        Some(Self {
            until: now.saturating_add(seconds),
        })
    }

    /// Where this one stands now.
    #[must_use]
    pub const fn standing(&self, now: u64) -> Standing {
        if now < self.until {
            Standing::InForce(self.until - now)
        } else {
            Standing::Expired(now - self.until)
        }
    }
}

impl Standing {
    /// Whether the limits are lifted right now.
    #[must_use]
    pub(crate) const fn lifting(self) -> bool {
        matches!(self, Self::InForce(_))
    }

    /// Whether the record behind this has done its work and should go.
    ///
    /// Read apart from [`Self::lifting`] deliberately: one decides what the limits
    /// are and the other decides what the record on disk should be, and folding
    /// them would mean an expiry that was cleared before it was ever reported.
    #[must_use]
    pub const fn spent(self) -> bool {
        matches!(self, Self::Expired(_))
    }

    /// What this means, in the words it is shown in.
    #[must_use]
    pub fn says(self) -> Option<String> {
        match self {
            Self::None => None,
            Self::InForce(seconds) => Some(format!(
                "Limits are lifted for another {}. They come back on their own.",
                crate::spoken::duration(seconds)
            )),
            Self::Expired(seconds) => Some(format!(
                "The limits you lifted came back {} ago, as they were always going to.",
                crate::spoken::duration(seconds)
            )),
        }
    }
}

#[cfg(test)]
mod tests;
