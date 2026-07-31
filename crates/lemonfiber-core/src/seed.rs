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
//! Every write is read back to confirm it landed, and recorded as a change. That
//! record is groundwork, not a live undo: it captures what a future service-side
//! reversal would need — the resource created and the id the service gave it — but
//! seeding itself is idempotent, so re-running it is how a partial run is
//! recovered, and nothing reverses a seed today. The journal each pass records
//! into is therefore not persisted; `recover`'s reversal is for an interrupted
//! setup apply, and cannot remove a resource a service created in any case.
//!
//! An interruption — a killed run, a service dropping mid-pass — therefore leaves
//! every connection already made intact and valid: each was written straight to
//! the service and read back before it was called done, and each is independent of
//! the rest, so a later run finds the completed ones already present, matched by
//! their own details, and leaves them untouched while it makes the ones the
//! interruption never reached. No connection depends on a later one having landed.
//!
//! A pass against a healthy stack is quick, well inside the minute the setup owes:
//! it is a bounded set of calls, one small group per service, with no wait or retry
//! inside a pass — a service that is not ready is skipped and left to the next run,
//! not polled until it comes up. Each call is bounded by the transport's own connect
//! and request timeouts, so even a service that hangs costs only a bounded wait —
//! a timeout per call it does not answer — before it is set aside rather than
//! stalling the run without end. Against a healthy stack every call answers at once,
//! so a pass's time tracks the number of services; only an unhealthy one pays those
//! timeouts to discover it is not answering.
//!
//! The policy — given what was observed about a connection, what seed intends —
//! is pure and settled without a service. The driver carries it out: it observes
//! the service through the port, registers what is missing, reads it back before
//! calling it done, and records each write as a change. The driver reaches the
//! outside only through the port, so it too runs against a fake.

use std::collections::BTreeMap;
use std::future::Future;

use serde::Serialize;

use crate::journal::{Change, Journal, Kind};
use crate::ports::random::Random;
use crate::ports::service::{
    AppSync, Application, Client, DownloadClient, Failure, MediaServer, RegisteredClient, Requests,
    RootFolder,
};
use crate::qbittorrent::Qbittorrent;
use crate::secret;

/// The administrator account name lemonfiber creates on the media server and
/// signs Seerr in with. Fixed, and recorded beside the password it mints.
const ADMIN: &str = "admin";

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
    /// Refused by lemonfiber's own policy, carrying the reason a re-run will not
    /// resolve — such as two \*arrs pointed at one root folder, or a service that
    /// does not serve the API version this build speaks. Either way nothing it
    /// names was written, whether it was refused before the write or the write
    /// itself found nothing to land in.
    Refused {
        /// Why it was refused, in lemonfiber's own words.
        reason: String,
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

    /// The connections not settled — the skipped, the failed and the refused —
    /// so the report says exactly what is left rather than only that something is.
    #[must_use]
    pub fn outstanding(&self) -> Vec<&Wiring> {
        self.wirings
            .iter()
            .filter(|wiring| !wiring.state.is_settled())
            .collect()
    }

    /// The connections refused by policy — a conflict a re-run will not lift,
    /// unlike the skipped and failed that [`Self::outstanding`] also holds. Named
    /// apart so the operator is told to resolve the clash rather than merely to
    /// run again, and so a script can tell "fix your config" from "retry".
    #[must_use]
    pub fn refused(&self) -> Vec<&Wiring> {
        self.wirings
            .iter()
            .filter(|wiring| matches!(wiring.state, State::Refused { .. }))
            .collect()
    }
}

/// Wire a service's root folders: register the ones it lacks, leave the ones it
/// already has, and record each write as a change.
///
/// The service is observed once. If it is not answering, every folder is skipped
/// so a later run completes them rather than any being called broken; if it
/// refuses, they fail. A folder the service already has is left alone — matched
/// by path, so a second run writes nothing. Each folder that must be written is
/// read back before it is called wired, because a write is not done until the
/// service reports it, and only then is it recorded.
///
/// A folder another \*arr also wants — named in `contested` (from
/// [`contested_roots`]) — is refused rather than written, because two \*arrs on
/// one root folder would each manage the other's files. A folder outside `root`,
/// the data tree lemonfiber mounts, is refused too: the service would file where
/// its downloads are neither hardlinked to nor visible to the rest of the stack.
/// Both refusals are made only once the service is reachable, so a service still
/// starting is skipped and retried rather than handed a verdict a re-run cannot
/// lift.
pub async fn wire_root_folders(
    client: &dyn Client,
    service: &str,
    wanted: &[RootFolder],
    contested: &BTreeMap<String, Vec<String>>,
    root: &str,
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
        let state = if let Some(reason) = contest_reason(service, folder, contested) {
            State::Refused { reason }
        } else if let Some(reason) = outside_root_reason(folder, root) {
            State::Refused { reason }
        } else if already {
            State::AlreadyWired
        } else {
            wire_one(
                client.register_root_folder(folder),
                client.root_folders(),
                |rows| {
                    rows.iter()
                        .find(|have| same_path(&have.path, &folder.path))
                        .map(|have| have.id.clone())
                },
                Naming {
                    service,
                    resource: "rootfolder",
                    noun: "folder",
                },
                journal,
                at,
            )
            .await
        };
        wirings.push(Wiring {
            connection: describe(service, folder),
            state,
        });
    }
    wirings
}

/// Register one connection, confirm it landed by reading the list back, and
/// record it as a change — the shared body of wiring a folder, a download client,
/// or an application.
///
/// The three differ only in the calls that register and read the list back, how a
/// landed row is matched to what was wanted and its id read off, and the nouns for
/// the journal and the read-back failure; those are the parameters, so the
/// register → read-back → confirm → record shape is written once. The two futures
/// are made by the caller and awaited here — register first, the read-back only if
/// it succeeded — which is free, because a future is inert until it is polled and
/// this keeps the ordering, and the two `unreached` gates, in one place.
async fn wire_one<T>(
    register: impl Future<Output = Result<(), Failure>>,
    read_back: impl Future<Output = Result<Vec<T>, Failure>>,
    landed: impl FnOnce(&[T]) -> Option<String>,
    naming: Naming<'_>,
    journal: &mut Journal,
    at: &str,
) -> State {
    if let Err(failure) = register.await {
        return unreached(&failure);
    }
    let registered = match read_back.await {
        Ok(rows) => rows,
        Err(failure) => return unreached(&failure),
    };
    match landed(&registered) {
        Some(id) => {
            journal.record(Change {
                at: at.to_owned(),
                operation: "seed".to_owned(),
                target: naming.service.to_owned(),
                kind: Kind::Created {
                    resource: naming.resource.to_owned(),
                    id,
                },
            });
            State::Wired
        }
        None => State::Failed {
            detail: format!(
                "the {} was accepted but did not appear when read back",
                naming.noun
            ),
        },
    }
}

/// How a wired connection is named where it is recorded and where it is missed —
/// the three labels that are all that differ between wiring a folder, a client and
/// an application once the register-and-read-back shape is shared.
struct Naming<'a> {
    /// The service the connection is recorded against.
    service: &'a str,
    /// The resource kind, as the journal stores it — `rootfolder`, and so on.
    resource: &'a str,
    /// The noun a read-back miss names the connection by, for the operator.
    noun: &'a str,
}

/// Wire a service's download clients: register the ones it lacks, leave the ones
/// it already has, and record each write as a change.
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
            Intent::Wire => {
                wire_one(
                    client.register_download_client(want),
                    client.download_clients(),
                    |rows| {
                        rows.iter()
                            .find(|have| same_endpoint(have, want))
                            .map(|have| have.id.clone())
                    },
                    Naming {
                        service,
                        resource: "downloadclient",
                        noun: "download client",
                    },
                    journal,
                    at,
                )
                .await
            }
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
/// the ones it already has, and record each write as a change.
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
            wire_one(
                prowlarr.register_application(application),
                prowlarr.applications(),
                |rows| {
                    rows.iter()
                        .find(|have| same_base_url(&have.base_url, &application.base_url))
                        .map(|have| have.id.clone())
                },
                Naming {
                    service,
                    resource: "application",
                    noun: "application",
                },
                journal,
                at,
            )
            .await
        };
        wirings.push(Wiring {
            connection: describe_application(service, application),
            state,
        });
    }
    wirings
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

/// Make Jellyfin the identity source for Seerr: mint and set Jellyfin's admin
/// account where its wizard has not run, then point Seerr's authentication at it.
///
/// Two services in order. Jellyfin has no key to read, so — like qBittorrent —
/// its admin password is one lemonfiber mints, sets by driving the first-run
/// wizard, and hands back for the surface to record; a wizard already run by the
/// household leaves its password unknown, so the wiring is skipped rather than
/// reset. With the credential in hand, Seerr is signed in through Jellyfin, which
/// on a fresh Seerr also creates its owner. An already-initialised Seerr is never
/// re-pointed, since that would cost the household its existing sign-ins. The
/// minted password is returned to record whenever the account was created, even
/// if Seerr itself could not then be reached, because the account now holds it.
pub async fn wire_jellyfin_identity(
    jellyfin: &dyn MediaServer,
    seerr: &dyn Requests,
    random: &dyn Random,
    recorded_password: Option<&str>,
    server_url: &str,
) -> (Wiring, Option<String>) {
    let connection = "Jellyfin as Seerr's identity".to_owned();
    let (password, minted) = match jellyfin_admin(jellyfin, random, recorded_password).await {
        Ok(pair) => pair,
        Err(state) => return (Wiring { connection, state }, None),
    };
    let state = configure_seerr(seerr, &password, server_url).await;
    (Wiring { connection, state }, minted)
}

/// The Jellyfin admin credential, and the password to record if it was newly
/// minted: minted where the wizard has not run, read from what was recorded where
/// it has, and unknown — so the wiring cannot proceed — where the household ran
/// the wizard itself.
async fn jellyfin_admin(
    jellyfin: &dyn MediaServer,
    random: &dyn Random,
    recorded: Option<&str>,
) -> Result<(String, Option<String>), State> {
    let completed = match jellyfin.startup_completed().await {
        Ok(done) => done,
        Err(failure) => return Err(unreached(&failure)),
    };
    if completed {
        return match recorded {
            Some(password) => Ok((password.to_owned(), None)),
            None => Err(State::Skipped {
                reason: "Jellyfin was set up outside lemonfiber, so its admin password is unknown; a later run cannot complete this until it is set up through lemonfiber".to_owned(),
            }),
        };
    }
    let Some(password) = secret::generate(random) else {
        return Err(State::Failed {
            detail: "no randomness was available to generate a password".to_owned(),
        });
    };
    match jellyfin.create_admin(ADMIN, &password).await {
        Ok(()) => Ok((password.clone(), Some(password))),
        Err(failure) => Err(unreached(&failure)),
    }
}

/// Point Seerr at the media server, unless it is already initialised — which is
/// left untouched, whether lemonfiber initialised it on an earlier run or the
/// household set it up with accounts of its own. A fresh Seerr is signed in and
/// then read back: it must report itself initialised, or the write did not land.
async fn configure_seerr(seerr: &dyn Requests, password: &str, server_url: &str) -> State {
    let initialized = match seerr.initialized().await {
        Ok(done) => done,
        Err(failure) => return unreached(&failure),
    };
    if initialized {
        return State::AlreadyWired;
    }
    if let Err(failure) = seerr.configure_identity(ADMIN, password, server_url).await {
        return unreached(&failure);
    }
    match seerr.initialized().await {
        Ok(true) => State::Wired,
        Ok(false) => State::Failed {
            detail: "Seerr accepted the sign-in but did not report itself initialised".to_owned(),
        },
        Err(failure) => unreached(&failure),
    }
}

/// A failure as the state it leaves a connection in: a service not answering is
/// skipped and retried; one that refuses is a failure, carrying its own words; one
/// that does not serve this build's API version is refused, since a re-run against
/// the same service will not resolve it — the operator must align the versions.
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
        Failure::Unsupported { service, detail } => State::Refused {
            reason: format!("{service} does not serve the API version this build speaks: {detail}"),
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
    canonical_root(a) == canonical_root(b)
}

/// A root-folder path in the form paths are compared in — the trailing separator
/// dropped, as [`same_path`] drops it — so one folder wanted by two \*arrs keys to
/// a single entry however each spells it.
fn canonical_root(path: &str) -> String {
    path.trim_end_matches('/').to_owned()
}

/// Root-folder paths more than one \*arr wants, each mapped to the \*arrs that
/// want it, named and sorted. Two \*arrs pointed at one root folder would each
/// manage the other's files, so a shared folder is refused rather than wired; a
/// path only one \*arr wants is left out, since there is nothing to refuse.
#[must_use]
pub fn contested_roots<'a>(
    claims: impl IntoIterator<Item = (&'a str, &'a [RootFolder])>,
) -> BTreeMap<String, Vec<String>> {
    let mut by_path: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (service, folders) in claims {
        for folder in folders {
            let services = by_path.entry(canonical_root(&folder.path)).or_default();
            if !services.iter().any(|name| name == service) {
                services.push(service.to_owned());
            }
        }
    }
    for services in by_path.values_mut() {
        services.sort();
    }
    by_path.retain(|_, services| services.len() > 1);
    by_path
}

/// Why a wanted folder is refused: the other \*arrs that also claim its path,
/// named, or `None` where the path is this \*arr's alone. Called only for a
/// folder this \*arr wants, so where the path is contested this \*arr is one of
/// its claimants and at least one other remains to name.
fn contest_reason(
    service: &str,
    folder: &RootFolder,
    contested: &BTreeMap<String, Vec<String>>,
) -> Option<String> {
    let others: Vec<&str> = contested
        .get(&canonical_root(&folder.path))?
        .iter()
        .map(String::as_str)
        .filter(|name| *name != service)
        .collect();
    Some(format!(
        "{} is also the root folder for {}; two *arrs on one root folder would each manage the other's files",
        folder.path,
        others.join(" and ")
    ))
}

/// Why a wanted folder is refused for falling outside the data root: its path, or
/// `None` where it sits within `root` — as every folder lemonfiber builds does,
/// under the tree it mounts at `root`. A root folder outside that tree would have
/// the service file where its downloads are neither hardlinked to nor visible to
/// the rest of the stack, so it is refused rather than created.
fn outside_root_reason(folder: &RootFolder, root: &str) -> Option<String> {
    let within = format!("{}/", canonical_root(root));
    if canonical_root(&folder.path).starts_with(&within) {
        return None;
    }
    Some(format!(
        "{} is outside the data root {root}; a root folder there is neither hardlinked to nor visible to the rest of the stack",
        folder.path
    ))
}

/// A connection's description for the report.
fn describe(service: &str, folder: &RootFolder) -> String {
    format!("{} root folder in {service}", folder.media_type)
}
