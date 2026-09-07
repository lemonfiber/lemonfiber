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
mod tests {
    use super::{Propagation, Reach, Rotation, Settled};

    /// A landed rotation over three consumers, one of which could not be reached.
    fn partly_landed() -> Rotation {
        Rotation::landed(
            "qBittorrent web UI password",
            "signed in with the replacement",
            vec![
                Propagation::updated("qBittorrent's own web UI"),
                Propagation::pending("the forwarded-port push", "lemonfiber restart torrent"),
                Propagation::failed("Sonarr's download client", "Sonarr did not answer"),
            ],
        )
    }

    #[test]
    fn a_refused_replacement_leaves_the_existing_credential_in_force() {
        let stopped = Rotation::stopped(
            "Indexer API key",
            Settled::Refused {
                detail: "the indexer refused it".to_owned(),
            },
        );

        assert!(stopped.kept_the_existing());
        assert!(stopped.consumers.is_empty());
    }

    #[test]
    fn an_unproven_replacement_leaves_the_existing_credential_in_force() {
        let stopped = Rotation::stopped(
            "Indexer API key",
            Settled::Unproven {
                detail: "nothing answered".to_owned(),
            },
        );

        assert!(stopped.kept_the_existing());
    }

    #[test]
    fn a_name_nothing_answers_to_changes_nothing_and_says_what_would_have() {
        let stopped = Rotation::stopped(
            "the wifi password",
            Settled::Unknown {
                known: vec!["Indexer API key".to_owned()],
            },
        );

        assert!(stopped.kept_the_existing());
        assert!(matches!(
            stopped.settled,
            Settled::Unknown { ref known } if known == &["Indexer API key".to_owned()]
        ));
    }

    #[test]
    fn a_credential_the_operators_provider_issued_is_not_one_lemonfiber_invents() {
        let stopped = Rotation::stopped(
            "Usenet provider password",
            Settled::Elsewhere {
                detail: "change it with your provider first".to_owned(),
            },
        );

        assert!(stopped.kept_the_existing());
    }

    #[test]
    fn only_a_landed_replacement_stops_the_existing_value_being_the_one_in_force() {
        assert!(!partly_landed().kept_the_existing());
    }

    #[test]
    fn a_consumer_that_could_not_be_updated_is_named_rather_than_passed_over() {
        let landed = partly_landed();

        assert_eq!(landed.stranded(), vec!["Sonarr's download client"]);
        assert_eq!(landed.consumers.len(), 3);
    }

    #[test]
    fn a_consumer_waiting_on_one_more_step_counts_as_carrying_the_replacement() {
        assert!(Reach::Updated.carried());
        assert!(Reach::Pending {
            detail: "lemonfiber restart torrent".to_owned()
        }
        .carried());
        assert!(!Reach::Failed {
            detail: "did not answer".to_owned()
        }
        .carried());
    }

    #[test]
    fn a_rotation_that_reached_everything_strands_nobody() {
        let landed = Rotation::landed(
            "Jellyfin administrator password",
            "signed in",
            vec![Propagation::updated("Jellyfin")],
        );

        assert_eq!(landed.consumers.len(), 1);
        assert!(landed.stranded().is_empty());
    }
}
