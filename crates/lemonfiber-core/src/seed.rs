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
use crate::ports::random::Random;
use crate::ports::service::{
    AppSync, Application, Client, DownloadClient, Failure, RegisteredClient, RootFolder,
};
use crate::qbittorrent::Qbittorrent;
use crate::secret;

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

/// Wire a service's download clients: register the ones it lacks, leave the ones
/// it already has, and record each write so it can be undone.
///
/// The same shape as [`wire_root_folders`], and the same two gates — an
/// unanswering service skips every client so a later run completes them, a
/// refusal fails. The difference is what "already there" means: a client is
/// matched by the endpoint it reaches, its host and port, not by its label, so a
/// client the operator renamed is recognised as the same connection and left
/// alone rather than registered a second time under lemonfiber's name.
pub async fn wire_download_clients(
    client: &dyn Client,
    service: &str,
    wanted: &[DownloadClient],
    journal: &mut Journal,
    at: &str,
) -> Vec<Wiring> {
    let existing = match client.download_clients().await {
        Ok(clients) => clients,
        Err(failure) => {
            let state = unreached(&failure);
            return wanted
                .iter()
                .map(|want| Wiring {
                    connection: describe_client(service, want),
                    state: state.clone(),
                })
                .collect();
        }
    };

    let mut wirings = Vec::new();
    for want in wanted {
        // The policy decides from what was observed: a client already there and
        // correct is left, one the operator re-filed is preserved, an absent one
        // is written. Unavailable never reaches here — a read-back failure was
        // handled above — so it folds harmlessly onto `Leave`.
        let state = match intent(observe_client(&existing, want)) {
            Intent::Wire => wire_one_client(client, service, want, journal, at).await,
            Intent::Preserve => State::Drifted,
            Intent::Leave | Intent::Skip => State::AlreadyWired,
        };
        wirings.push(Wiring {
            connection: describe_client(service, want),
            state,
        });
    }
    wirings
}

/// What a seed pass observes about a wanted client against what the service
/// already holds: absent, present and correct, or present but re-filed by the
/// operator under a different category.
fn observe_client(existing: &[RegisteredClient], want: &DownloadClient) -> Observed {
    match existing.iter().find(|have| same_endpoint(have, want)) {
        None => Observed::Absent,
        Some(have) if drifted(have, want) => Observed::Drifted,
        Some(_) => Observed::Present,
    }
}

/// Whether a client at the wanted endpoint files under a different category than
/// seed intends. Only a category the service reported can drift; where it names
/// none there is nothing to compare, so the connection is taken as it stands.
fn drifted(have: &RegisteredClient, want: &DownloadClient) -> bool {
    have.category
        .as_ref()
        .is_some_and(|category| category != &want.category)
}

/// Register one download client, confirm it landed by reading it back, and record
/// it.
async fn wire_one_client(
    client: &dyn Client,
    service: &str,
    want: &DownloadClient,
    journal: &mut Journal,
    at: &str,
) -> State {
    if let Err(failure) = client.register_download_client(want).await {
        return unreached(&failure);
    }
    let registered = match client.download_clients().await {
        Ok(clients) => clients,
        Err(failure) => return unreached(&failure),
    };
    match registered
        .into_iter()
        .find(|have| same_endpoint(have, want))
    {
        Some(landed) => {
            journal.record(Change {
                at: at.to_owned(),
                operation: "seed".to_owned(),
                target: service.to_owned(),
                kind: Kind::Created {
                    resource: "downloadclient".to_owned(),
                    id: landed.id,
                },
            });
            State::Wired
        }
        None => State::Failed {
            detail: "the download client was accepted but did not appear when read back".to_owned(),
        },
    }
}

/// Whether a registered client reaches the same endpoint as a wanted one.
///
/// The host and port together, never the name: this is what makes the match by
/// connection rather than by label, so a client already present under a different
/// name is not registered again.
fn same_endpoint(have: &RegisteredClient, want: &DownloadClient) -> bool {
    have.host == want.host && have.port == want.port
}

/// A download-client connection's description for the report.
fn describe_client(service: &str, client: &DownloadClient) -> String {
    format!("{} into {service}", client.name)
}

/// Wire Prowlarr's applications: register the media-filing \*arrs it lacks, leave
/// the ones it already has, and record each write so it can be undone.
///
/// The same shape as [`wire_root_folders`], matched by the address Prowlarr
/// reaches an \*arr on rather than by a label, so an application an operator
/// renamed is recognised as the same connection and not registered a second time.
/// An application already present is left exactly as it is and never rewritten,
/// which is what preserves an operator's own change to its sync settings.
pub async fn wire_applications(
    prowlarr: &dyn AppSync,
    service: &str,
    wanted: &[Application],
    journal: &mut Journal,
    at: &str,
) -> Vec<Wiring> {
    let existing = match prowlarr.applications().await {
        Ok(applications) => applications,
        Err(failure) => {
            let state = unreached(&failure);
            return wanted
                .iter()
                .map(|application| Wiring {
                    connection: describe_application(service, application),
                    state: state.clone(),
                })
                .collect();
        }
    };

    let mut wirings = Vec::new();
    for application in wanted {
        let already = existing
            .iter()
            .any(|have| same_base_url(&have.base_url, &application.base_url));
        let state = if already {
            State::AlreadyWired
        } else {
            wire_one_application(prowlarr, service, application, journal, at).await
        };
        wirings.push(Wiring {
            connection: describe_application(service, application),
            state,
        });
    }
    wirings
}

/// Register one application, confirm it landed by reading it back, and record it.
async fn wire_one_application(
    prowlarr: &dyn AppSync,
    service: &str,
    application: &Application,
    journal: &mut Journal,
    at: &str,
) -> State {
    if let Err(failure) = prowlarr.register_application(application).await {
        return unreached(&failure);
    }
    let registered = match prowlarr.applications().await {
        Ok(applications) => applications,
        Err(failure) => return unreached(&failure),
    };
    match registered
        .into_iter()
        .find(|have| same_base_url(&have.base_url, &application.base_url))
    {
        Some(landed) => {
            journal.record(Change {
                at: at.to_owned(),
                operation: "seed".to_owned(),
                target: service.to_owned(),
                kind: Kind::Created {
                    resource: "application".to_owned(),
                    id: landed.id,
                },
            });
            State::Wired
        }
        None => State::Failed {
            detail: "the application was accepted but did not appear when read back".to_owned(),
        },
    }
}

/// Whether two application addresses reach the same \*arr.
///
/// A trailing separator is ignored, as it is for a folder path: Prowlarr may
/// store the canonical form of the address it was given, and the wanted address
/// and its stored form must be recognised as one so a write is not made every run.
fn same_base_url(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

/// An application connection's description for the report.
fn describe_application(service: &str, application: &Application) -> String {
    format!("{} indexer sync via {service}", application.name)
}

/// Replace qBittorrent's temporary web UI password with a generated one, and hand
/// the generated value back so the surface can record it where the forwarded-port
/// push reads it.
///
/// Unlike every other connection, this one is a credential lemonfiber mints
/// rather than reads. Generating it needs randomness the operating system might
/// withhold; without it there is nothing to set, and the connection fails rather
/// than falling back to a guessable secret on the client the forwarded port
/// authenticates to. The client sets the password and confirms it by
/// authenticating again; only a confirmed change is wired, and only then is the
/// value returned to record — an unset or unconfirmed one records nothing.
pub async fn wire_qbittorrent_password(
    client: &Qbittorrent,
    random: &dyn Random,
    temporary: &str,
) -> (Wiring, Option<String>) {
    let connection = "qBittorrent web UI password".to_owned();
    let Some(password) = secret::generate(random) else {
        return (
            Wiring {
                connection,
                state: State::Failed {
                    detail: "no randomness was available to generate a password".to_owned(),
                },
            },
            None,
        );
    };

    match client.replace_password(temporary, &password).await {
        Ok(()) => (
            Wiring {
                connection,
                state: State::Wired,
            },
            Some(password),
        ),
        Err(failure) => (
            Wiring {
                connection,
                state: unreached(&failure),
            },
            None,
        ),
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
