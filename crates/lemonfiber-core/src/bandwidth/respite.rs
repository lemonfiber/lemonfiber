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
    pub const fn lifting(self) -> bool {
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
mod tests {
    use super::{Respite, Standing, LONGEST};

    /// A moment every case here reads against.
    const NOW: u64 = 1_790_812_800;

    #[test]
    fn a_respite_cannot_be_asked_for_without_an_end() {
        // The whole design. There is no way to ask for one that does not stop,
        // so there is nothing to leave switched on.
        assert_eq!(Respite::asked_for(NOW, 0), None);
        assert_eq!(Respite::asked_for(NOW, LONGEST + 1), None);
        assert_eq!(Respite::asked_for(NOW, 24 * 60 * 60), None);
        assert_eq!(
            Respite::asked_for(NOW, LONGEST),
            Some(Respite {
                until: NOW + LONGEST
            }),
            "the longest one may ask for is still one that may be asked for"
        );
    }

    #[test]
    fn one_still_running_says_how_long_it_has_and_that_it_ends_itself() {
        let respite = Respite {
            until: NOW + 45 * 60,
        };
        assert_eq!(respite.standing(NOW), Standing::InForce(45 * 60));
        assert!(respite.standing(NOW).lifting());
        assert!(!respite.standing(NOW).spent());
        let said = respite.standing(NOW).says().unwrap_or_default();
        assert!(said.contains("45 minutes"), "{said}");
        assert!(said.contains("come back on their own"), "{said}");
    }

    #[test]
    fn one_that_ran_out_is_reported_before_it_is_cleared() {
        // An operator wondering why the download slowed down is owed the reason,
        // and the reason is that the thing they asked for finished.
        let respite = Respite {
            until: NOW - 30 * 60,
        };
        assert_eq!(respite.standing(NOW), Standing::Expired(30 * 60));
        assert!(!respite.standing(NOW).lifting());
        assert!(respite.standing(NOW).spent());
        let said = respite.standing(NOW).says().unwrap_or_default();
        assert!(said.contains("came back"), "{said}");
        assert!(said.contains("30 minutes"), "{said}");
    }

    #[test]
    fn the_moment_it_ends_it_has_ended() {
        assert_eq!(Respite { until: NOW }.standing(NOW), Standing::Expired(0));
    }

    #[test]
    fn no_respite_has_nothing_to_say() {
        assert_eq!(Standing::None.says(), None);
        assert!(!Standing::None.lifting());
        assert!(!Standing::None.spent());
    }
}
