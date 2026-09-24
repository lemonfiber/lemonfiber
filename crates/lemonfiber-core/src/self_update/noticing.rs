//! How often the release list is asked, and when to stop asking it.
//!
//! Two rules, and both exist to keep a check nobody asked for from becoming a thing
//! the operator notices. The first is that asking is rare: a version that came out
//! this morning is no more useful to know about now than in an hour, so the answer
//! is remembered and reused, and running the same read twice reaches the network
//! once. The second is that failing gets quieter rather than louder — a machine with
//! no route out would otherwise reach for the network on every run for ever, which
//! is the shape a thing has when it is about to be described as spyware.
//!
//! Failing enough times in a row stops it entirely. That is deliberate and it is the
//! only state here with no way back on its own: a laptop that has been off the
//! network for a week should not still be trying, and the operator who wants an
//! answer anyway has a way to say so.

use serde::{Deserialize, Serialize};

/// How long to leave between asking, once.
const APART: u64 = 60 * 60 * 24;

/// How many failures in a row before it stops being attempted at all.
const GIVEN_UP: u32 = 5;

/// The longest the wait is allowed to double to, in failures.
///
/// Beyond this the wait stops growing, which matters only because the arithmetic
/// would otherwise leave the realm of numbers a machine holds. Reached before
/// [`GIVEN_UP`] would stop it anyway, so this is a bound rather than a policy.
const DOUBLINGS: u32 = 4;

/// What the last checks came to, kept between runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Noticed {
    /// When the release list was last asked, in seconds since the epoch.
    asked: Option<u64>,
    /// The newest version the last answered check read.
    offered: Option<String>,
    /// What that version's release page said it changed, where it said anything.
    ///
    /// Kept beside the version rather than fetched when it is wanted, because the one
    /// answer the check already reads holds both — and a machine that has gone quiet
    /// should be able to say what the version it knows about brought, not only that
    /// it exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    changed: Option<String>,
    /// The manifest generation that version's stack carries, where it declared one.
    ///
    /// Remembered with the version for the same reason the notes are: a machine that
    /// has gone quiet should still be able to say what the update it knows about would
    /// bring, and re-asking to learn it would be a second request for an answer that
    /// has already arrived once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    schema: Option<u32>,
    /// How many checks in a row have not been answered.
    quiet: u32,
}

impl Noticed {
    /// Whether the release list should be asked now.
    ///
    /// A machine that has never asked always should. Otherwise the wait since the
    /// last attempt has to have passed, and enough failures in a row means it never
    /// should again — which is what "stops being attempted" comes to, and is why
    /// this is the one answer here that does not come back on its own.
    #[must_use]
    pub fn due(&self, now: u64) -> bool {
        if self.quiet >= GIVEN_UP {
            return false;
        }
        let Some(asked) = self.asked else {
            return true;
        };
        now.saturating_sub(asked) >= self.wait()
    }

    /// How long to leave before asking again, given how the last few went.
    ///
    /// Doubling per failure, so a machine with no route out is asking once a day,
    /// then every other day, then every fourth. A check that answers puts it back to
    /// once a day, because the reason for the long wait has gone.
    fn wait(&self) -> u64 {
        APART << self.quiet.min(DOUBLINGS)
    }

    /// Record a check that was answered, and what it read.
    ///
    /// The notes move with the version they belong to and never on their own: a
    /// version remembered from one check beside notes from another would be a report
    /// describing a release nobody is being offered.
    pub fn answered(
        &mut self,
        now: u64,
        offered: Option<String>,
        changed: Option<String>,
        schema: Option<u32>,
    ) {
        self.asked = Some(now);
        self.quiet = 0;
        if offered.is_some() {
            self.offered = offered;
            self.changed = changed;
            self.schema = schema;
        }
    }

    /// Record a check that nothing answered.
    pub fn silent(&mut self, now: u64) {
        self.asked = Some(now);
        self.quiet = self.quiet.saturating_add(1);
    }

    /// The newest version the last answered check read, where one has.
    #[must_use]
    pub fn remembered(&self) -> Option<&str> {
        self.offered.as_deref()
    }

    /// What that version's release page said it changed, where it said anything.
    #[must_use]
    pub fn changed(&self) -> Option<&str> {
        self.changed.as_deref()
    }

    /// The manifest generation that version's stack carries, where it declared one.
    #[must_use]
    pub const fn schema(&self) -> Option<u32> {
        self.schema
    }

    /// Whether asking has been given up on, which is worth saying rather than
    /// leaving as an answer that never changes.
    #[must_use]
    pub(crate) const fn given_up(&self) -> bool {
        self.quiet >= GIVEN_UP
    }
}

/// Why availability could not be told, where it could not.
///
/// Four reasons, and they differ in what an operator would do about each: one is a
/// setting they chose, one is a machine that has stopped asking, one is an address
/// that said nothing, and one is a check that is simply not due. None of them is a
/// fault, and none of them stops anything — which is why this is a reason rather than
/// a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Silence {
    /// The operator switched the check off.
    Refused,
    /// Enough checks in a row went unanswered that it is no longer attempted.
    GivenUp,
    /// The address was asked and said nothing this could read a version out of.
    Unanswered,
    /// Nothing has been read yet and it is not time to ask again.
    NotYet,
}

impl Silence {
    /// What to say about it, and what to do about it where there is anything.
    #[must_use]
    pub const fn why(self) -> &'static str {
        match self {
            Self::Refused => {
                "The release list is not asked, because this machine's settings say so. \
                 Nothing else changes: every other thing lemonfiber does works exactly as \
                 well without knowing whether a version came out."
            }
            Self::GivenUp => {
                "Enough checks in a row went unanswered that this machine has stopped \
                 asking. That is deliberate — a laptop with no route out should not reach \
                 for the network on every run for ever — and it starts again once a check \
                 is answered."
            }
            Self::Unanswered => {
                "The release list did not answer, or answered with nothing a version could \
                 be read out of. Nothing waits on it, and the next attempt is further off \
                 than the last one was."
            }
            Self::NotYet => {
                "The release list has not been asked yet today, and nothing has been read \
                 from it before. A version that came out this morning is no more useful to \
                 know about now than in an hour."
            }
        }
    }
}

#[cfg(test)]
mod tests;
