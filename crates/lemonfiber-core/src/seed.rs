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
//! The policy — given what was observed about a connection, what seed intends —
//! is pure and settled without a service. The driver carries it out: it observes
//! the service through the port, registers what is missing, reads it back before
//! calling it done, and records each write so it can be undone. The driver
//! reaches the outside only through the port, so it too runs against a fake.

use serde::Serialize;

use crate::journal::{Change, Journal, Kind};
use crate::ports::service::{Client, Failure, RootFolder};

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

/// Wire a service's root folders: register the ones it lacks, leave the ones it
/// already has, and record each write so it can be undone.
///
/// The service is observed once. If it is not answering, every folder is skipped
/// so a later run completes them rather than any being called broken; if it
/// refuses, they fail. A folder the service already has is left alone — matched
/// by path, so a second run writes nothing. Each folder that must be written is
/// read back before it is called wired, because a write is not done until the
/// service reports it, and only then is it recorded.
pub async fn wire_root_folders(
    client: &dyn Client,
    service: &str,
    wanted: &[RootFolder],
    journal: &mut Journal,
    at: &str,
) -> Vec<Wiring> {
    let existing = match client.root_folders().await {
        Ok(folders) => folders,
        Err(failure) => {
            let state = unreached(&failure);
            return wanted
                .iter()
                .map(|folder| Wiring {
                    connection: describe(service, folder),
                    state: state.clone(),
                })
                .collect();
        }
    };

    let mut wirings = Vec::new();
    for folder in wanted {
        let already = existing
            .iter()
            .any(|have| same_path(&have.path, &folder.path));
        let state = if already {
            State::AlreadyWired
        } else {
            wire_one(client, service, folder, journal, at).await
        };
        wirings.push(Wiring {
            connection: describe(service, folder),
            state,
        });
    }
    wirings
}

/// Register one folder, confirm it landed by reading it back, and record it.
async fn wire_one(
    client: &dyn Client,
    service: &str,
    folder: &RootFolder,
    journal: &mut Journal,
    at: &str,
) -> State {
    if let Err(failure) = client.register_root_folder(folder).await {
        return unreached(&failure);
    }
    let registered = match client.root_folders().await {
        Ok(folders) => folders,
        Err(failure) => return unreached(&failure),
    };
    match registered
        .into_iter()
        .find(|have| same_path(&have.path, &folder.path))
    {
        Some(landed) => {
            journal.record(Change {
                at: at.to_owned(),
                operation: "seed".to_owned(),
                target: service.to_owned(),
                kind: Kind::Created {
                    resource: "rootfolder".to_owned(),
                    id: landed.id,
                },
            });
            State::Wired
        }
        None => State::Failed {
            detail: "the folder was accepted but did not appear when read back".to_owned(),
        },
    }
}

/// A failure as the state it leaves a connection in: a service not answering is
/// skipped and retried; one that refuses is a failure, carrying its own words.
fn unreached(failure: &Failure) -> State {
    match failure {
        Failure::Unavailable { .. } => State::Skipped {
            reason: "the service is not answering; a later run will complete it".to_owned(),
        },
        Failure::Unauthorised { service } => State::Failed {
            detail: format!("{service} refused the credential"),
        },
        Failure::Refused { detail, .. } => State::Failed {
            detail: detail.clone(),
        },
    }
}

/// Whether two paths name the same folder.
///
/// A trailing separator is ignored, because a service commonly stores the
/// canonical form of the path it was given — dropping a trailing slash — and the
/// wanted path and its stored canonical form must be recognised as one. Without
/// this, a folder is re-registered every run and read-back never confirms the
/// write it just made.
fn same_path(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

/// A connection's description for the report.
fn describe(service: &str, folder: &RootFolder) -> String {
    format!("{} root folder in {service}", folder.media_type)
}
