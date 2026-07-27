//! Wiring the services to each other.
//!
//! Two gates before anything is written: a service that is not answering is
//! skipped rather than failed, so the whole run stays resumable; and a value the
//! operator changed themselves is preserved rather than reverted.
//!
//! That second gate is what makes seeding safe to run against a stack somebody
//! has tuned by hand, and it is the resolution of the standing tension between
//! reproducible and customised.
//!
//! Every write is read back to confirm it landed, and recorded so it can be
//! undone.
//!
//! What lives here is the policy, kept apart from the doing of it: given what was
//! observed about a connection, what seed intends; and, once each connection has
//! been carried out, what the whole pass amounted to and what a re-run still owes.
//! Observing the services and carrying the intent out is the driver's part, above
//! this; deciding is pure, and tested without a service.

use serde::Serialize;

/// What lemonfiber observed about a connection it wants to make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observed {
    /// The prerequisite service is not answering.
    Unavailable,
    /// The connection is not there.
    Absent,
    /// The connection is there and matches lemonfiber's baseline.
    Present,
    /// The connection is there but differs from the baseline — an operator edit.
    Drifted,
}

/// What seed will do about a connection, decided from what it observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// The prerequisite is not up: skip it, and a later run completes it.
    Skip,
    /// It is not there: write it.
    Wire,
    /// It is there and correct: do nothing — this is what keeps a second run from
    /// changing anything.
    Leave,
    /// It is there but the operator changed it: preserve their change and report
    /// the drift, rather than reverting it.
    Preserve,
}

/// What seed will do about a connection found in the given state.
///
/// The whole of the idempotent, drift-aware policy in one place: an absent
/// connection is written, a present-and-correct one is left untouched so a second
/// run changes nothing, an operator's edit is preserved, and an unavailable
/// prerequisite is skipped rather than failed so the run stays resumable.
#[must_use]
pub const fn intent(observed: Observed) -> Intent {
    match observed {
        Observed::Unavailable => Intent::Skip,
        Observed::Absent => Intent::Wire,
        Observed::Present => Intent::Leave,
        Observed::Drifted => Intent::Preserve,
    }
}

/// How one connection turned out after a seed pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum State {
    /// Written and read back.
    Wired,
    /// Present and correct; nothing was done.
    AlreadyWired,
    /// Present but operator-changed; preserved.
    Drifted,
    /// Prerequisite unavailable; a later run will complete it.
    Skipped {
        /// Why it could not be attempted.
        reason: String,
    },
    /// Attempted and rejected, carrying the service's own words.
    Failed {
        /// What the service said.
        detail: String,
    },
}

impl State {
    /// Whether this connection is settled: wired one way or another, or left as
    /// the operator's own. A skip or a failure is not settled and a re-run must
    /// return to it.
    #[must_use]
    pub const fn is_settled(&self) -> bool {
        matches!(self, Self::Wired | Self::AlreadyWired | Self::Drifted)
    }
}

/// One connection, and how it turned out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Wiring {
    /// What was being connected, such as `SABnzbd into Sonarr`.
    pub connection: String,
    /// How it turned out.
    pub state: State,
}

/// What a seed pass amounted to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Every connection attempted, and how each turned out.
    pub wirings: Vec<Wiring>,
}

impl Report {
    /// Whether every connection is settled, so nothing needs a re-run.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.wirings.iter().all(|wiring| wiring.state.is_settled())
    }

    /// The connections a re-run still owes — the skipped and the failed — so the
    /// report says exactly what is left rather than only that something is.
    #[must_use]
    pub fn outstanding(&self) -> Vec<&Wiring> {
        self.wirings
            .iter()
            .filter(|wiring| !wiring.state.is_settled())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{intent, Intent, Observed, Report, State, Wiring};

    #[test]
    fn the_policy_follows_from_what_was_observed() {
        assert_eq!(intent(Observed::Unavailable), Intent::Skip);
        assert_eq!(intent(Observed::Absent), Intent::Wire);
        // Idempotent: a connection already correct is left, so a second run writes
        // nothing.
        assert_eq!(intent(Observed::Present), Intent::Leave);
        // Drift-aware: an operator's own change is preserved, never reverted.
        assert_eq!(intent(Observed::Drifted), Intent::Preserve);
    }

    #[test]
    fn only_a_wired_or_preserved_connection_is_settled() {
        for settled in [State::Wired, State::AlreadyWired, State::Drifted] {
            assert!(settled.is_settled(), "{settled:?} is settled");
        }
        for unsettled in [
            State::Skipped {
                reason: "lidarr is not in the active form".to_owned(),
            },
            State::Failed {
                detail: "rejected".to_owned(),
            },
        ] {
            assert!(!unsettled.is_settled(), "{unsettled:?} is not settled");
        }
    }

    fn wiring(connection: &str, state: State) -> Wiring {
        Wiring {
            connection: connection.to_owned(),
            state,
        }
    }

    #[test]
    fn a_pass_where_everything_settled_is_complete() {
        let report = Report {
            wirings: vec![
                wiring("SABnzbd into Sonarr", State::Wired),
                wiring("root folder in Radarr", State::AlreadyWired),
            ],
        };
        assert!(report.is_complete());
        assert!(report.outstanding().is_empty());
    }

    #[test]
    fn a_skip_or_a_failure_leaves_a_pass_incomplete_and_named() {
        let report = Report {
            wirings: vec![
                wiring("SABnzbd into Sonarr", State::Wired),
                wiring(
                    "SABnzbd into Lidarr",
                    State::Skipped {
                        reason: "lidarr is not running".to_owned(),
                    },
                ),
                wiring(
                    "qBittorrent into Radarr",
                    State::Failed {
                        detail: "rejected".to_owned(),
                    },
                ),
            ],
        };
        assert!(!report.is_complete());
        let outstanding: Vec<&str> = report
            .outstanding()
            .into_iter()
            .map(|wiring| wiring.connection.as_str())
            .collect();
        assert_eq!(
            outstanding,
            vec!["SABnzbd into Lidarr", "qBittorrent into Radarr"],
            "a re-run is told exactly what it still owes"
        );
    }

    #[test]
    fn the_report_names_each_state_on_the_wire() {
        let report = Report {
            wirings: vec![
                wiring("a", State::Wired),
                wiring("b", State::AlreadyWired),
                wiring("c", State::Drifted),
                wiring(
                    "d",
                    State::Skipped {
                        reason: "later".to_owned(),
                    },
                ),
                wiring(
                    "e",
                    State::Failed {
                        detail: "no".to_owned(),
                    },
                ),
            ],
        };
        let json = serde_json::to_string(&report).unwrap_or_default();
        for state in ["wired", "already-wired", "drifted", "skipped", "failed"] {
            assert!(json.contains(&format!(r#""state":"{state}""#)), "{json}");
        }
    }
}
