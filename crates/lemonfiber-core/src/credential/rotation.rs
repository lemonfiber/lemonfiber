//! What became of a replacement, and of every consumer it had to reach.
//!
//! The ordering this reports on is the whole of why rotation is a first-class
//! operation rather than an edit: a replacement is proven against the live service
//! *before* the existing value stops being the one in force, so a mistyped key
//! leaves the operator with a working credential rather than with neither. Every
//! outcome here except one therefore keeps the existing value, and
//! [`Rotation::kept_the_existing`] is that property written down where a test can
//! hold it.
//!
//! The consumer list is reported in full, including the ones that could not be
//! reached. A rotation that reached three of four consumers and said "done" is the
//! failure this reporting exists to prevent — the fourth one goes on authenticating
//! with a value that no longer works, and nothing anywhere says so.

use serde::Serialize;

/// What became of a rotation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "settled", rename_all = "kebab-case")]
pub enum Settled {
    /// The replacement was proven and is now the value in force.
    Replaced {
        /// What the service did while proving it — an observation, never the value.
        observed: String,
    },
    /// The service answered and refused the replacement. Nothing was changed.
    Refused {
        /// What the service said, with any credential in it withheld.
        detail: String,
    },
    /// Nothing usable answered, so the replacement could not be proven. Nothing
    /// was changed, because an unproven replacement is not a better one.
    Unproven {
        /// Why nothing could be concluded.
        detail: String,
    },
    /// Nothing was attempted, because this run only said what a rotation would do.
    ///
    /// Its own outcome rather than one of the refusals above, because it is not a
    /// refusal: nothing went wrong, and what an operator is being told is what would
    /// happen if they ran it again meaning it. Carrying its own three fields rather
    /// than one sentence, because "it would rotate the qBittorrent password" is not a
    /// report — where the value lives is what would be written over, and what is owed
    /// afterwards is the half nobody finds out about until a consumer stops working.
    ///
    /// No value appears here and none is generated to put here. A replacement minted
    /// to describe a rotation is a secret that exists because somebody asked a
    /// question, and it would then have to be kept or thrown away — and one thrown
    /// away may be one the service has already taken.
    Rehearsed {
        /// What a real run would do, step by step, in lemonfiber's own words.
        detail: String,
        /// Where the value that would be replaced is kept.
        location: String,
        /// What would still need doing before every consumer held the replacement.
        afterwards: Vec<String>,
    },
    /// Nothing in this stack holds a credential by that name.
    Unknown {
        /// The names that would have been accepted.
        known: Vec<String>,
    },
    /// A replacement for this one does not come from here.
    ///
    /// Either the operator's provider issued it, in which case inventing one would
    /// produce a credential no service has ever heard of; or the service that holds
    /// it offers no way to change it in place. Either way what is owed is a
    /// sentence saying where a replacement does come from, not an attempt.
    Elsewhere {
        /// Where a replacement comes from, and what to do once it exists.
        detail: String,
    },
}

/// How far a rotation reached one consumer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "reach", rename_all = "kebab-case")]
#[schemars(rename = "CredentialReach")]
pub enum Reach {
    /// It now holds the replacement.
    Updated,
    /// It will hold the replacement once one more thing happens, and that thing is
    /// named. A consumer reading the value out of a container's environment has it
    /// fixed at the moment the container was created, so recording a new one is
    /// only half of reaching it.
    Pending {
        /// What still has to happen, written as the command that does it.
        detail: String,
    },
    /// It could not be updated. Named rather than dropped, because a consumer left
    /// holding the old value is the failure this list exists to surface.
    Failed {
        /// Why it could not be.
        detail: String,
    },
}

impl Reach {
    /// Whether this consumer is now, or will be, holding the replacement.
    #[must_use]
    pub const fn carried(&self) -> bool {
        matches!(self, Self::Updated | Self::Pending { .. })
    }
}

/// One consumer, and how far the rotation reached it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Propagation {
    /// What authenticates with the credential.
    pub consumer: String,
    /// How far the rotation reached it.
    pub reach: Reach,
}

impl Propagation {
    /// A consumer now holding the replacement.
    #[must_use]
    pub fn updated(consumer: &str) -> Self {
        Self {
            consumer: consumer.to_owned(),
            reach: Reach::Updated,
        }
    }

    /// A consumer that will hold it once one more named thing happens.
    #[must_use]
    pub fn pending(consumer: &str, detail: &str) -> Self {
        Self {
            consumer: consumer.to_owned(),
            reach: Reach::Pending {
                detail: detail.to_owned(),
            },
        }
    }

    /// A consumer that could not be reached.
    #[must_use]
    pub fn failed(consumer: &str, detail: &str) -> Self {
        Self {
            consumer: consumer.to_owned(),
            reach: Reach::Failed {
                detail: detail.to_owned(),
            },
        }
    }
}

/// What one rotation came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Rotation {
    /// Which credential was to be replaced.
    pub credential: String,
    /// What became of the replacement.
    pub settled: Settled,
    /// Every consumer, and how far the rotation reached it.
    pub consumers: Vec<Propagation>,
}

impl Rotation {
    /// A rotation that got no further than the attempt: nothing was replaced, so no
    /// consumer was touched.
    #[must_use]
    pub fn stopped(credential: &str, settled: Settled) -> Self {
        Self {
            credential: credential.to_owned(),
            settled,
            consumers: Vec::new(),
        }
    }

    /// A rotation this run only said it would make.
    ///
    /// No consumers, for the reason [`Self::stopped`] carries none: nothing was
    /// replaced, so nothing was reached. What would be owed afterwards travels inside
    /// the outcome instead, where it reads as something still to do rather than as a
    /// consumer that is already holding a new value.
    #[must_use]
    pub fn would(credential: &str, detail: &str, location: &str, afterwards: Vec<String>) -> Self {
        Self {
            credential: credential.to_owned(),
            settled: Settled::Rehearsed {
                detail: detail.to_owned(),
                location: location.to_owned(),
                afterwards,
            },
            consumers: Vec::new(),
        }
    }

    /// A rotation whose replacement was proven, with what became of each consumer.
    #[must_use]
    pub fn landed(credential: &str, observed: &str, consumers: Vec<Propagation>) -> Self {
        Self {
            credential: credential.to_owned(),
            settled: Settled::Replaced {
                observed: observed.to_owned(),
            },
            consumers,
        }
    }

    /// Whether the credential that was in force before this rotation is still the
    /// one in force.
    ///
    /// True for every outcome but a landed replacement, and that is the guarantee: a
    /// replacement that was refused, that could not be proven, or that named nothing
    /// leaves the operator exactly where they were rather than with nothing working.
    #[must_use]
    pub const fn kept_the_existing(&self) -> bool {
        !matches!(self.settled, Settled::Replaced { .. })
    }

    /// Whether this run only said what a rotation would do.
    ///
    /// Read where an outcome is scored, and the reason it is asked apart from
    /// [`Self::kept_the_existing`]: every other way of not replacing something is a
    /// rotation that was asked for and did not land, which is a failure worth a
    /// non-zero exit. This one was never asked to replace anything.
    #[must_use]
    pub const fn rehearsed(&self) -> bool {
        matches!(self.settled, Settled::Rehearsed { .. })
    }

    /// Every consumer the rotation could not update.
    ///
    /// Read off the same list the report carries, so what is named here and what is
    /// shown cannot disagree.
    #[must_use]
    pub fn stranded(&self) -> Vec<&str> {
        self.consumers
            .iter()
            .filter(|one| !one.reach.carried())
            .map(|one| one.consumer.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests;
