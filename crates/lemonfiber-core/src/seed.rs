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

use crate::baseline::Baseline;
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

/// What lemonfiber observed about a connection it wants to make — the outcome of
/// the three-way comparison of what it last wrote, what the service now holds, and
/// what it would write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observed {
    /// The prerequisite service is not answering.
    Unavailable,
    /// The connection is not there.
    Absent,
    /// The connection is there and already at what lemonfiber would write.
    Present,
    /// It differs from the baseline while lemonfiber's intent is unchanged — an
    /// operator's edit, to preserve.
    Drifted,
    /// It still holds lemonfiber's baseline value, but lemonfiber's intent has moved
    /// on — its own old value, to bring up to date.
    Stale,
    /// Both the service's value and lemonfiber's intent have moved away from the
    /// baseline — a conflict lemonfiber must not resolve on its own.
    Conflicted,
    /// The service holds a value the operator set and lemonfiber adopted as the
    /// accepted state — theirs to keep, left as it is.
    Adopted,
    /// The service holds a value lemonfiber never wrote and has no baseline for — the
    /// operator's own, pre-existing, to adopt as the baseline rather than report as
    /// drift from an expectation that was never formed.
    Unmanaged,
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
    /// It is lemonfiber's own value, now behind lemonfiber's intent: it should be
    /// brought up to date. Reported until an update path exists to apply it.
    Update,
    /// Both sides changed: present the conflict and leave the value, never resolving
    /// it silently.
    Ask,
    /// An adopted operator value still stands: keep it, changing nothing — the
    /// baseline already holds it as the accepted state.
    Keep,
    /// A pre-existing operator value with no baseline: adopt what the service holds
    /// as the baseline, writing nothing to the service.
    Adopt,
}

/// What seed will do about a connection found in the given state.
///
/// The idempotent, drift-aware policy in one place: an absent connection is
/// written, a present-and-correct one is left untouched so a second run changes
/// nothing, an operator's edit is preserved, lemonfiber's own value behind its
/// intent is brought up to date, a conflict is presented rather than resolved, an
/// adopted edit is kept as the accepted state, a pre-existing value with no baseline
/// is adopted rather than reported as drift, and an unavailable prerequisite is
/// skipped rather than failed so the run stays resumable.
#[must_use]
pub const fn intent(observed: Observed) -> Intent {
    match observed {
        Observed::Unavailable => Intent::Skip,
        Observed::Absent => Intent::Wire,
        Observed::Present => Intent::Leave,
        Observed::Drifted => Intent::Preserve,
        Observed::Stale => Intent::Update,
        Observed::Conflicted => Intent::Ask,
        Observed::Adopted => Intent::Keep,
        Observed::Unmanaged => Intent::Adopt,
    }
}

/// The three-way comparison of one managed value: what lemonfiber last recorded
/// (`expected`, `None` where it recorded nothing — value and whether it was written
/// or adopted), what the service now holds (`actual`, `None` where it holds no
/// value), and what lemonfiber would write (`desired`).
///
/// Like a merge, the three together say what a bare two-way `actual != desired`
/// cannot: whether a difference is the operator's edit to preserve, lemonfiber's
/// own value fallen behind its intent, an adopted value to keep, or a genuine
/// conflict. Where there is no baseline to judge against, a value the service holds
/// is the operator's own, pre-existing and unmanaged — adopted rather than
/// overwritten on a guess.
#[must_use]
pub fn reconcile(
    expected: Option<&crate::baseline::Record>,
    actual: Option<&str>,
    desired: &str,
) -> Observed {
    // An adopted value is read before "already at desired": a value the operator
    // adopted that lemonfiber also happens to want stays adopted — theirs to keep
    // even once lemonfiber's desired later moves — rather than being taken back as
    // lemonfiber's own. While the service still holds it, it stands; moved away from
    // it, in sync if now at desired, otherwise a fresh edit to preserve.
    if let Some(base) = expected {
        if base.origin.is_adopted() {
            return if actual == Some(base.value.as_str()) {
                Observed::Adopted
            } else if actual == Some(desired) {
                Observed::Present
            } else {
                Observed::Drifted
            };
        }
    }
    if actual == Some(desired) {
        return Observed::Present;
    }
    match expected {
        // lemonfiber's intent is unchanged from the baseline, so the difference is
        // the operator's.
        Some(base) if base.value == desired => Observed::Drifted,
        // The service still holds lemonfiber's baseline value; only lemonfiber's
        // intent has moved.
        Some(base) if actual == Some(base.value.as_str()) => Observed::Stale,
        // A baseline that matches neither side: both have moved away from it.
        Some(_) => Observed::Conflicted,
        // No baseline to judge against: the value the service holds is the operator's
        // own, never written by lemonfiber, to adopt rather than overwrite on a guess.
        None => Observed::Unmanaged,
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
    /// Present and still lemonfiber's own value, but behind lemonfiber's intent —
    /// it should be brought up to date. Reported until an update path applies it,
    /// and never overwritten in the meantime.
    Stale,
    /// Both the service's value and lemonfiber's intent moved away from the
    /// baseline. The conflict is presented — the value the operator set beside the
    /// one lemonfiber would write — and the value left as it is; lemonfiber does not
    /// resolve it on its own.
    ///
    /// Both values are shown in the report and serialized with it, so a
    /// secret-bearing field must not report a conflict through this variant: a
    /// conflict in a secret is to be reported without either value on show, and
    /// wants a masked shape of its own rather than this one.
    Conflicted {
        /// The value the service now holds, as the operator set it; `None` where
        /// they cleared it.
        yours: Option<String>,
        /// The value lemonfiber would write in its place.
        ours: String,
    },
    /// An operator's edit adopted as the accepted state, kept across seeds and
    /// restores. Settled: lemonfiber leaves it as it is.
    Adopted,
    /// A value the service already held that lemonfiber never wrote — the operator's
    /// own, pre-existing. Adopted as the baseline this run rather than reported as
    /// drift, so an existing setup is taken on instead of flagged wholesale. Its
    /// value is not shown, so a secret among the adopted is never put on display.
    Unmanaged,
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
    /// Whether this connection is settled: wired one way or another, or left in a
    /// working state — the operator's own edit, an adopted or pre-existing value that
    /// is theirs to keep, or lemonfiber's own value that is merely behind its newer
    /// intent. A skip, a failure, a refusal or a conflict is
    /// not settled: a re-run or an operator's decision must return to it.
    #[must_use]
    pub const fn is_settled(&self) -> bool {
        matches!(
            self,
            Self::Wired
                | Self::AlreadyWired
                | Self::Drifted
                | Self::Stale
                | Self::Adopted
                | Self::Unmanaged
        )
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

    /// The connections a re-run will not lift — refused by policy, or a conflict
    /// lemonfiber will not resolve on its own — unlike the skipped and failed that
    /// [`Self::outstanding`] also holds. Named apart so the operator is told to
    /// resolve the clash rather than merely to run again, and so a script can tell
    /// "fix your config" from "retry".
    #[must_use]
    pub fn blocked(&self) -> Vec<&Wiring> {
        self.wirings
            .iter()
            .filter(|wiring| {
                matches!(
                    wiring.state,
                    State::Refused { .. } | State::Conflicted { .. }
                )
            })
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
    let existing = match observe_or_skip(client.root_folders().await, wanted, |folder| {
        describe(service, folder)
    }) {
        Ok(existing) => existing,
        Err(skipped) => return skipped,
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

/// The baseline a drift-aware wiring reads against and records into, and whether
/// this is an adopt pass. Grouped so the wiring call carries one baseline argument
/// rather than three: what lemonfiber last recorded, where this run records what it
/// leaves, and whether an operator's edit is promoted to adopted rather than merely
/// preserved.
pub struct Baselines<'a> {
    /// What lemonfiber last recorded — the expected leg of the comparison.
    pub expected: &'a Baseline,
    /// Where this run records what it leaves as the baseline a later run reads.
    pub records: &'a mut Baseline,
    /// Whether this pass promotes each drifted value to adopted, recording what the
    /// service holds as the operator's accepted state.
    pub adopt: bool,
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
    baselines: &mut Baselines<'_>,
    at: &str,
) -> Vec<Wiring> {
    // The shared expected reference copies out; the mutable `records` stays reached
    // through `baselines` so the two do not both borrow it at once.
    let expected = baselines.expected;
    let adopt = baselines.adopt;
    let existing = match observe_or_skip(client.download_clients().await, wanted, |want| {
        describe_client(service, want)
    }) {
        Ok(existing) => existing,
        Err(skipped) => return skipped,
    };

    let mut wirings = Vec::new();
    for want in wanted {
        // What lemonfiber last recorded here, if anything — value and whether it was
        // written or adopted — the expected leg of the three-way comparison, read from
        // the snapshot this run loaded, separate from the `records` it writes into so
        // the two do not fight over one borrow.
        let field = client_field(want);
        let base = expected.entry(service, &field);
        // Match the wanted client to one the service already holds by the endpoint it
        // reaches, found once so the observation, the value to present on a conflict,
        // and the value to adopt are all read from the same client.
        let have = existing.iter().find(|have| same_endpoint(have, want));
        let found = have
            .and_then(|have| have.category.as_ref())
            .map(|category| category.value.clone());
        let observed = observe_client(have, want, base);
        // Adoption is a deliberate act, so only an adopt pass takes a value on: it
        // promotes what an ordinary seed would merely preserve as drift, and takes on
        // what a seed would only report as unmanaged, recording the value the service
        // holds below as the accepted baseline. An ordinary seed reports both and
        // records neither — it never claims the operator's value as lemonfiber's on
        // its own, so a lost baseline is not silently frozen as adopted. There must be
        // a value to adopt: a client the operator cleared has none, so it stays what it
        // was rather than being reported adopted when nothing was taken on.
        let adopting =
            adopt && found.is_some() && matches!(observed, Observed::Drifted | Observed::Unmanaged);
        // The policy decides from the three-way comparison: an absent client is
        // written, one already at lemonfiber's value is left, an operator's edit is
        // preserved, lemonfiber's own value behind its intent is reported stale, a
        // two-sided change is presented as a conflict, an adopted value is kept, and a
        // pre-existing value with no baseline is reported unmanaged. Unavailable never
        // reaches here — a read-back failure was handled above — so it folds onto
        // `Leave`.
        let state = if adopting {
            State::Adopted
        } else {
            match intent(observed) {
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
                Intent::Update => State::Stale,
                // Present both sides of the conflict: the value the operator set, drawn
                // from the client already matched above, beside the one lemonfiber
                // would write. The value is left as it is — presenting is not resolving.
                Intent::Ask => State::Conflicted {
                    yours: found.clone(),
                    ours: want.category.value.clone(),
                },
                Intent::Keep => State::Adopted,
                Intent::Adopt => State::Unmanaged,
                Intent::Leave | Intent::Skip => State::AlreadyWired,
            }
        };
        // Record the expected state this run leaves. A client lemonfiber wrote
        // (`Wired`) or one already at what it wants (`AlreadyWired`) records the
        // category lemonfiber set. A value taken on by an adopt pass — an edit promoted
        // or a pre-existing value adopted — records what the service holds, as the
        // operator's, marked adopted so a later run keeps it. Everything else leaves
        // the baseline as it was: a drift or unmanaged value an ordinary seed only
        // reports, a conflict, an already-adopted value the loaded baseline still
        // carries, or a categoryless client with nothing to record.
        if matches!(state, State::Wired | State::AlreadyWired) {
            baselines
                .records
                .record(service, &field, &want.category.value, at);
        } else if adopting {
            if let Some(value) = &found {
                baselines.records.adopt(service, &field, value, at);
            }
        }
        wirings.push(Wiring {
            connection: describe_client(service, want),
            state,
        });
    }
    wirings
}

/// What a seed pass observes about a wanted client against the one the service
/// holds at its endpoint (`have`, `None` where it holds none) and what lemonfiber
/// last recorded: absent, or the three-way outcome of comparing the service's
/// category with lemonfiber's baseline and its desired one.
fn observe_client(
    have: Option<&RegisteredClient>,
    want: &DownloadClient,
    expected: Option<&crate::baseline::Record>,
) -> Observed {
    match have {
        None => Observed::Absent,
        Some(have) => reconcile(
            expected,
            have.category
                .as_ref()
                .map(|category| category.value.as_str()),
            &want.category.value,
        ),
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

/// The baseline field a download client's value is recorded under: the endpoint it
/// reaches, host and port, so it is keyed the way the client itself is matched —
/// by connection, not by the label an operator can rename.
fn client_field(client: &DownloadClient) -> String {
    format!("downloadclient:{}:{}", client.host, client.port)
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
    let existing = match observe_or_skip(prowlarr.applications().await, wanted, |application| {
        describe_application(service, application)
    }) {
        Ok(existing) => existing,
        Err(skipped) => return skipped,
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

/// A service's existing resources, observed once — or, where it could not be
/// reached, every wanted connection as the state that unreachability leaves it in.
/// Each driver opens the same way; this is that opening, so the three do not each
/// repeat it: `Ok` yields what the service holds, `Err` yields the wirings to
/// return in its place, described by the caller's own namer.
fn observe_or_skip<T, W>(
    observed: Result<Vec<T>, Failure>,
    wanted: &[W],
    describe: impl Fn(&W) -> String,
) -> Result<Vec<T>, Vec<Wiring>> {
    observed.map_err(|failure| {
        let state = unreached(&failure);
        wanted
            .iter()
            .map(|want| Wiring {
                connection: describe(want),
                state: state.clone(),
            })
            .collect()
    })
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
