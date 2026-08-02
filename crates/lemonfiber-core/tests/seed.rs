//! The seed policy and the driver that carries it out.
//!
//! The policy is pure and could be tested in place; the driver speaks the service
//! port, an async trait, so both are driven from here — where a fake service
//! stands in and the driver's coverage is counted from the one compiled copy.

use std::collections::BTreeMap;
use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_core::baseline::{Baseline, Origin, Record};
use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::random::Random;
use lemonfiber_core::ports::service::{
    AppSync, Application, ApplicationKind, Category, Client, ClientKind, ClientProbe, Credential,
    DownloadClient, Failure, Identity, MediaServer, RegisteredApplication, RegisteredClient,
    RegisteredFolder, Requests, RootFolder,
};
use lemonfiber_core::seed::{
    contested_roots, intent, reconcile, wholesale_drift, wire_applications, wire_download_clients,
    wire_jellyfin_identity, wire_root_folders, Assessment, Baselines, Intent, Observed, Report,
    Severity, State, Wiring,
};

// ---- The policy: pure, decided without a service. ----

#[test]
fn the_policy_follows_from_what_was_observed() {
    assert_eq!(intent(Observed::Unavailable), Intent::Skip);
    assert_eq!(intent(Observed::Absent), Intent::Wire);
    // Idempotent: a connection already correct is left, so a second run writes
    // nothing.
    assert_eq!(intent(Observed::Present), Intent::Leave);
    // Drift-aware: an operator's own change is preserved, never reverted.
    assert_eq!(intent(Observed::Drifted), Intent::Preserve);
    // lemonfiber's own value behind its intent is brought up to date.
    assert_eq!(intent(Observed::Stale), Intent::Update);
    // A two-sided change is presented, never resolved on lemonfiber's own.
    assert_eq!(intent(Observed::Conflicted), Intent::Ask);
    // An adopted value is kept; a pre-existing one with no baseline is adopted.
    assert_eq!(intent(Observed::Adopted), Intent::Keep);
    assert_eq!(intent(Observed::Unmanaged), Intent::Adopt);
}

#[test]
fn the_three_way_comparison_reads_every_row_of_the_merge_table() {
    // The expected leg carries a value and where it came from; a helper builds each.
    let written = |value: &str| Record {
        value: value.to_owned(),
        at: "1".to_owned(),
        origin: Origin::Written,
    };
    let adopted = |value: &str| Record {
        value: value.to_owned(),
        at: "1".to_owned(),
        origin: Origin::Adopted,
    };
    // Actual already at desired: in sync, whoever moved it there.
    assert_eq!(
        reconcile(Some(&written("tv")), Some("tv"), "tv"),
        Observed::Present
    );
    assert_eq!(
        reconcile(Some(&written("old")), Some("tv"), "tv"),
        Observed::Present
    );
    // Actual differs, but lemonfiber's intent is unchanged from the baseline: the
    // operator's edit, preserved.
    assert_eq!(
        reconcile(Some(&written("tv")), Some("mine"), "tv"),
        Observed::Drifted
    );
    // Actual still at the baseline, only lemonfiber's intent moved: stale, its own.
    assert_eq!(
        reconcile(Some(&written("tv")), Some("tv"), "tv-hd"),
        Observed::Stale
    );
    // Baseline matches neither side: both moved away — a conflict.
    assert_eq!(
        reconcile(Some(&written("tv")), Some("mine"), "tv-hd"),
        Observed::Conflicted
    );
    assert_eq!(
        reconcile(Some(&written("tv")), None, "tv-hd"),
        Observed::Conflicted
    );
    // An adopted value the service still holds is kept, even though lemonfiber's
    // desired differs — it is the operator's, not lemonfiber's own to bring up to
    // date; changed again, it is a fresh edit to preserve.
    assert_eq!(
        reconcile(Some(&adopted("mine")), Some("mine"), "tv"),
        Observed::Adopted
    );
    assert_eq!(
        reconcile(Some(&adopted("mine")), Some("other"), "tv"),
        Observed::Drifted
    );
    // An adopted value the operator moved to match lemonfiber's desired is in sync.
    assert_eq!(
        reconcile(Some(&adopted("mine")), Some("tv"), "tv"),
        Observed::Present
    );
    // No baseline to judge against: the value the service holds is the operator's
    // own, unmanaged — adopted rather than overwritten on a guess.
    assert_eq!(reconcile(None, Some("mine"), "tv"), Observed::Unmanaged);
    assert_eq!(reconcile(None, None, "tv"), Observed::Unmanaged);
}

#[test]
fn only_a_wired_preserved_or_stale_connection_is_settled() {
    // Settled: written, already correct, the operator's own edit, an adopted or
    // pre-existing value theirs to keep, or lemonfiber's own value merely behind its
    // intent — all working states.
    for settled in [
        State::Wired,
        State::AlreadyWired,
        State::Drifted,
        State::Stale,
        State::Adopted,
        State::Unmanaged,
    ] {
        assert!(settled.is_settled(), "{settled:?} is settled");
    }
    // Not settled: a re-run or an operator's decision must return to it.
    for unsettled in [
        State::Skipped {
            reason: "lidarr is not in the active form".to_owned(),
        },
        State::Failed {
            detail: "rejected".to_owned(),
        },
        State::Conflicted {
            yours: Some("mine".to_owned()),
            ours: "tv-hd".to_owned(),
        },
    ] {
        assert!(!unsettled.is_settled(), "{unsettled:?} is not settled");
    }
}

fn wiring(connection: &str, state: State) -> Wiring {
    Wiring::settled(connection.to_owned(), state)
}

#[test]
fn a_pass_where_everything_settled_is_complete() {
    let report = Report {
        assessment: Assessment::Assessed,
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
        assessment: Assessment::Assessed,
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
fn a_blocked_connection_is_named_apart_from_the_merely_outstanding() {
    // A refusal and a conflict are both outstanding like a skip, but a re-run will
    // not lift them — the operator must resolve them — so `blocked` names them apart
    // from a skip, which a later run does complete.
    let report = Report {
        assessment: Assessment::Assessed,
        wirings: vec![
            wiring("tv root folder in sonarr", State::Wired),
            wiring(
                "SABnzbd into Lidarr",
                State::Skipped {
                    reason: "lidarr is not running".to_owned(),
                },
            ),
            wiring(
                "movies root folder in radarr",
                State::Refused {
                    reason: "shared with sonarr".to_owned(),
                },
            ),
            wiring(
                "SABnzbd into Sonarr",
                State::Conflicted {
                    yours: Some("mine".to_owned()),
                    ours: "tv-hd".to_owned(),
                },
            ),
        ],
    };
    assert!(!report.is_complete());
    let blocked: Vec<&str> = report
        .blocked()
        .into_iter()
        .map(|wiring| wiring.connection.as_str())
        .collect();
    assert_eq!(
        blocked,
        vec!["movies root folder in radarr", "SABnzbd into Sonarr"],
        "a refusal and a conflict are named as blocked; the skip is not",
    );
}

#[test]
fn the_report_names_each_state_on_the_wire() {
    let report = Report {
        assessment: Assessment::Assessed,
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
            wiring("f", State::Stale),
            wiring(
                "g",
                State::Conflicted {
                    yours: Some("mine".to_owned()),
                    ours: "tv-hd".to_owned(),
                },
            ),
            wiring("h", State::Adopted),
            wiring("i", State::Unmanaged),
        ],
    };
    let json = serde_json::to_string(&report).unwrap_or_default();
    for state in [
        "wired",
        "already-wired",
        "drifted",
        "skipped",
        "failed",
        "stale",
        "conflicted",
        "adopted",
        "unmanaged",
    ] {
        assert!(json.contains(&format!(r#""state":"{state}""#)), "{json}");
    }
}

#[test]
fn the_report_draws_out_the_drifts_that_broke_the_stack() {
    // A drift raised to a warning is drawn out on its own, while an ordinary drift and
    // the settled connections are not — so a surface can lead with what must be acted
    // on. The warning also serialises its severity, breakage and remedy.
    let mut broken = wiring("broken", State::Drifted);
    broken.escalate(
        "the root folder points where nothing exists".to_owned(),
        "create the directory".to_owned(),
    );
    let report = Report {
        assessment: Assessment::Assessed,
        wirings: vec![
            broken,
            wiring("ordinary", State::Drifted),
            wiring("fine", State::Wired),
        ],
    };
    let warned: Vec<&str> = report
        .warnings()
        .iter()
        .map(|wiring| wiring.connection.as_str())
        .collect();
    assert_eq!(warned, vec!["broken"], "only the broken drift is a warning");
    let json = serde_json::to_string(&report).unwrap_or_default();
    assert!(json.contains(r#""severity":"warning""#), "{json}");
    assert!(json.contains(r#""severity":"informational""#), "{json}");
}

// ---- The driver: carried out against a fake service. ----

/// How the fake service behaves.
enum Mode {
    /// Answers normally; registering a folder adds it.
    Normal,
    /// Not answering — listing and registering both fail as unavailable.
    Down,
    /// Reachable but refuses the credential when listed.
    RefusesList,
    /// Reachable, lists fine, but rejects a registration with its own words.
    RejectsRegister,
    /// Accepts a registration but the folder never appears on read-back.
    Swallows,
    /// Lists fine to observe and registers fine, then stops answering on the
    /// read-back that would confirm the write.
    DropsAfterRegister,
    /// Reachable, but does not serve the API version this build speaks — the
    /// listing fails as unsupported, a conflict a re-run cannot lift.
    Unsupported,
    /// Lists fine, but refuses an in-place update — the failure a reset's revert meets
    /// when the service will not take lemonfiber's category back.
    RefusesUpdate,
}

/// A service that answers the seed driver from a script.
struct FakeService {
    mode: Mode,
    folders: Mutex<Vec<RegisteredFolder>>,
    clients: Mutex<Vec<RegisteredClient>>,
    reads: Mutex<u32>,
    next_id: Mutex<u32>,
    /// The client-test verdicts to answer with, or `None` to fail the test call —
    /// the signal a drift-severity escalation reads. Absent by default, since only a
    /// drift asks for it.
    probes: Mutex<Option<Vec<ClientProbe>>>,
}

impl FakeService {
    fn with(mode: Mode, folders: Vec<RegisteredFolder>) -> Self {
        Self {
            mode,
            folders: Mutex::new(folders),
            clients: Mutex::new(Vec::new()),
            reads: Mutex::new(0),
            next_id: Mutex::new(100),
            probes: Mutex::new(None),
        }
    }

    fn with_clients(mode: Mode, clients: Vec<RegisteredClient>) -> Self {
        Self {
            mode,
            folders: Mutex::new(Vec::new()),
            clients: Mutex::new(clients),
            reads: Mutex::new(0),
            next_id: Mutex::new(100),
            probes: Mutex::new(None),
        }
    }

    /// The client-test verdicts this service answers with when its clients are
    /// tested — how a drift is driven to reachable or unreachable.
    fn probing(self, probes: Vec<ClientProbe>) -> Self {
        Self {
            probes: Mutex::new(Some(probes)),
            ..self
        }
    }

    /// The folders the service is holding — what a run registered and the service
    /// kept, so a next run can be given the state an interrupted one left behind.
    fn registered(&self) -> Vec<RegisteredFolder> {
        self.folders
            .lock()
            .map(|folders| folders.clone())
            .unwrap_or_default()
    }
}

fn down(service: &str) -> Failure {
    Failure::Unavailable {
        service: service.to_owned(),
    }
}

#[async_trait]
impl Client for FakeService {
    async fn identity(&self) -> Result<Identity, Failure> {
        Ok(Identity {
            name: "sonarr".to_owned(),
            version: "4".to_owned(),
        })
    }

    async fn register_download_client(&self, client: &DownloadClient) -> Result<(), Failure> {
        match self.mode {
            Mode::Down => Err(down("sonarr")),
            Mode::RejectsRegister => Err(Failure::Refused {
                service: "sonarr".to_owned(),
                detail: "HTTP 400: unknown implementation".to_owned(),
            }),
            Mode::Swallows => Ok(()),
            _ => {
                if let (Ok(mut clients), Ok(mut id)) = (self.clients.lock(), self.next_id.lock()) {
                    clients.push(RegisteredClient {
                        id: id.to_string(),
                        host: client.host.clone(),
                        port: client.port,
                        category: Some(client.category.clone()),
                    });
                    *id += 1;
                }
                Ok(())
            }
        }
    }

    async fn update_download_client(
        &self,
        id: &str,
        client: &DownloadClient,
    ) -> Result<(), Failure> {
        if matches!(self.mode, Mode::Down) {
            return Err(down("sonarr"));
        }
        if matches!(self.mode, Mode::RefusesUpdate) {
            return Err(Failure::Refused {
                service: "sonarr".to_owned(),
                detail: "HTTP 400: cannot update".to_owned(),
            });
        }
        if let Ok(mut clients) = self.clients.lock() {
            if let Some(existing) = clients.iter_mut().find(|held| held.id == id) {
                existing.category = Some(client.category.clone());
            }
        }
        Ok(())
    }

    async fn test_download_clients(&self) -> Result<Vec<ClientProbe>, Failure> {
        // The set verdicts where they were given; a service with none set fails the
        // test, standing in for one that will not run it.
        self.probes
            .lock()
            .ok()
            .and_then(|probes| probes.clone())
            .map_or_else(|| Err(down("sonarr")), Ok)
    }

    async fn download_clients(&self) -> Result<Vec<RegisteredClient>, Failure> {
        let count = match self.reads.lock() {
            Ok(mut reads) => {
                *reads += 1;
                *reads
            }
            Err(_) => 0,
        };
        match self.mode {
            Mode::Down => Err(down("sonarr")),
            Mode::RefusesList => Err(Failure::Unauthorised {
                service: "sonarr".to_owned(),
            }),
            Mode::DropsAfterRegister if count >= 2 => Err(down("sonarr")),
            _ => Ok(self
                .clients
                .lock()
                .map(|clients| clients.clone())
                .unwrap_or_default()),
        }
    }

    async fn register_root_folder(&self, folder: &RootFolder) -> Result<(), Failure> {
        match self.mode {
            Mode::Down => Err(down("sonarr")),
            Mode::RejectsRegister => Err(Failure::Refused {
                service: "sonarr".to_owned(),
                detail: "HTTP 400: path is already used".to_owned(),
            }),
            Mode::Swallows => Ok(()),
            _ => {
                if let (Ok(mut folders), Ok(mut id)) = (self.folders.lock(), self.next_id.lock()) {
                    folders.push(RegisteredFolder {
                        id: id.to_string(),
                        // The service stores the canonical path, dropping a
                        // trailing slash — as the Servarr apps do.
                        path: folder.path.trim_end_matches('/').to_owned(),
                    });
                    *id += 1;
                }
                Ok(())
            }
        }
    }

    async fn root_folders(&self) -> Result<Vec<RegisteredFolder>, Failure> {
        let count = match self.reads.lock() {
            Ok(mut reads) => {
                *reads += 1;
                *reads
            }
            Err(_) => 0,
        };
        match self.mode {
            Mode::Down => Err(down("sonarr")),
            Mode::RefusesList => Err(Failure::Unauthorised {
                service: "sonarr".to_owned(),
            }),
            Mode::Unsupported => Err(Failure::Unsupported {
                service: "sonarr".to_owned(),
                detail: "there is no /api/v3".to_owned(),
            }),
            Mode::DropsAfterRegister if count >= 2 => Err(down("sonarr")),
            _ => Ok(self
                .folders
                .lock()
                .map(|folders| folders.clone())
                .unwrap_or_default()),
        }
    }
}

fn folder(path: &str) -> RootFolder {
    RootFolder {
        path: path.to_owned(),
        media_type: "tv".to_owned(),
    }
}

/// Run the driver for one wanted folder, returning its resulting state and the
/// number of changes journalled. No folder is contested by another \*arr.
async fn seed(service: FakeService, wanted: &[RootFolder]) -> (Vec<State>, usize) {
    seed_contested(service, wanted, &BTreeMap::new()).await
}

/// Run the driver with a set of contested root-folder paths, so a folder another
/// \*arr also claims is refused rather than wired.
async fn seed_contested(
    service: FakeService,
    wanted: &[RootFolder],
    contested: &BTreeMap<String, Vec<String>>,
) -> (Vec<State>, usize) {
    let mut journal = Journal::new();
    // Every folder a test wants is under `/data`, the mounted data root, so none is
    // refused for falling outside it unless the test deliberately reaches beyond.
    let wirings = wire_root_folders(
        &service,
        "sonarr",
        wanted,
        contested,
        "/data",
        &mut journal,
        "t",
    )
    .await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, journal.changes().len())
}

/// Drive the folder wiring against a borrowed service, so one fake can be carried
/// across two passes — standing in for the state a killed run leaves in the real
/// service between one run and the next. A fresh journal each pass, as production
/// keeps none across passes.
async fn wire_on(service: &FakeService, wanted: &[RootFolder]) -> Vec<State> {
    let mut journal = Journal::new();
    wire_root_folders(
        service,
        "sonarr",
        wanted,
        &BTreeMap::new(),
        "/data",
        &mut journal,
        "t",
    )
    .await
    .into_iter()
    .map(|wiring| wiring.state)
    .collect()
}

#[tokio::test]
async fn a_write_that_landed_before_an_interruption_is_not_duplicated_on_the_next_run() {
    // The load-bearing interruption: the write reached the service, but the run died
    // before the read-back could confirm it. `DropsAfterRegister` is exactly that —
    // the folder is registered (and kept by the service), then the confirming read
    // fails, so the pass leaves it outstanding rather than calling it done.
    let interrupted = FakeService::with(Mode::DropsAfterRegister, Vec::new());
    let first = wire_on(&interrupted, &[folder("/data/media/tv")]).await;
    assert!(
        matches!(first.as_slice(), [State::Skipped { .. }]),
        "an unconfirmed write is left outstanding, not called done: {first:?}"
    );

    // The service kept the folder the interrupted run registered — the state a
    // killed run leaves behind. The next run, now answering, must find it already
    // there and leave it, not register a second copy.
    let landed = interrupted.registered();
    assert_eq!(landed.len(), 1, "the write did land at the service");
    let (states, wrote) = seed(
        FakeService::with(Mode::Normal, landed),
        &[folder("/data/media/tv")],
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(
        wrote, 0,
        "the connection that survived the interruption is left intact, not duplicated"
    );
}

#[tokio::test]
async fn an_absent_folder_is_registered_read_back_and_recorded() {
    let (states, recorded) = seed(
        FakeService::with(Mode::Normal, Vec::new()),
        &[folder("/data/media/tv")],
    )
    .await;
    assert_eq!(states, vec![State::Wired]);
    assert_eq!(recorded, 1, "the write is journalled so it can be undone");
}

#[tokio::test]
async fn a_folder_the_service_already_has_is_left_untouched() {
    // Idempotent: the folder is present, so nothing is written or journalled.
    let existing = vec![RegisteredFolder {
        id: "1".to_owned(),
        path: "/data/media/tv".to_owned(),
    }];
    let (states, recorded) = seed(
        FakeService::with(Mode::Normal, existing),
        &[folder("/data/media/tv")],
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(recorded, 0, "an already-wired connection writes nothing");
}

#[tokio::test]
async fn a_wanted_path_is_matched_to_the_services_canonical_form() {
    // The service stores the path without its trailing slash. The read-back must
    // still recognise the folder it just registered, wire it, and record it —
    // rather than declaring the landed write a failure.
    let (states, recorded) = seed(
        FakeService::with(Mode::Normal, Vec::new()),
        &[folder("/data/media/tv/")],
    )
    .await;
    assert_eq!(states, vec![State::Wired]);
    assert_eq!(recorded, 1);
}

#[tokio::test]
async fn a_present_folder_is_matched_despite_a_trailing_slash() {
    // Idempotent across the same normalization: the service already holds the
    // canonical path, and a wanted path that differs only by a trailing slash is
    // left alone, not re-registered.
    let existing = vec![RegisteredFolder {
        id: "1".to_owned(),
        path: "/data/media/tv".to_owned(),
    }];
    let (states, recorded) = seed(
        FakeService::with(Mode::Normal, existing),
        &[folder("/data/media/tv/")],
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(recorded, 0);
}

/// The reason a shared root folder is refused, naming the given other \*arr, in
/// the exact words the driver builds so the assertions read against one source.
fn shared_root(path: &str, other: &str) -> String {
    format!(
        "{path} is also the root folder for {other}; two *arrs on one root folder would each manage the other's files"
    )
}

#[tokio::test]
async fn a_root_folder_another_arr_also_wants_is_refused_not_wired() {
    // Two *arrs on one root folder would each manage the other's files, so the
    // shared folder is refused — with the other *arr named — and nothing written.
    let contested = BTreeMap::from([(
        "/data/media/tv".to_owned(),
        vec!["radarr".to_owned(), "sonarr".to_owned()],
    )]);
    let (states, recorded) = seed_contested(
        FakeService::with(Mode::Normal, Vec::new()),
        &[folder("/data/media/tv")],
        &contested,
    )
    .await;
    assert_eq!(
        states,
        vec![State::Refused {
            reason: shared_root("/data/media/tv", "radarr"),
        }]
    );
    assert_eq!(recorded, 0, "a refused folder writes nothing");
}

#[tokio::test]
async fn a_contested_folder_is_refused_even_where_the_service_already_holds_it() {
    // The clash is the point: a folder already registered is still refused when
    // another *arr shares it, so the two are never left both managing one root.
    let existing = vec![RegisteredFolder {
        id: "1".to_owned(),
        path: "/data/media/tv".to_owned(),
    }];
    let contested = BTreeMap::from([(
        "/data/media/tv".to_owned(),
        vec!["radarr".to_owned(), "sonarr".to_owned()],
    )]);
    let (states, recorded) = seed_contested(
        FakeService::with(Mode::Normal, existing),
        &[folder("/data/media/tv")],
        &contested,
    )
    .await;
    assert_eq!(
        states,
        vec![State::Refused {
            reason: shared_root("/data/media/tv", "radarr"),
        }]
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn only_the_contested_folder_is_refused_the_rest_are_wired() {
    // Refusal is per folder: the shared one is refused, the *arr's own is wired.
    let contested = BTreeMap::from([(
        "/data/media/tv".to_owned(),
        vec!["radarr".to_owned(), "sonarr".to_owned()],
    )]);
    let (states, recorded) = seed_contested(
        FakeService::with(Mode::Normal, Vec::new()),
        &[folder("/data/media/tv"), folder("/data/media/movies")],
        &contested,
    )
    .await;
    assert_eq!(
        states,
        vec![
            State::Refused {
                reason: shared_root("/data/media/tv", "radarr"),
            },
            State::Wired,
        ]
    );
    assert_eq!(recorded, 1, "only the wired folder is journalled");
}

#[test]
fn a_root_folder_two_arrs_want_is_contested_naming_both_sorted() {
    let sonarr = [folder("/data/media/tv")];
    let radarr = [folder("/data/media/tv")];
    let contested = contested_roots([("sonarr", sonarr.as_slice()), ("radarr", radarr.as_slice())]);
    assert_eq!(
        contested.get("/data/media/tv").map(Vec::as_slice),
        Some(["radarr".to_owned(), "sonarr".to_owned()].as_slice()),
        "both *arrs are named, sorted so the reason reads the same either way",
    );
}

#[test]
fn a_root_folder_only_one_arr_wants_is_not_contested() {
    let sonarr = [folder("/data/media/tv")];
    let radarr = [folder("/data/media/movies")];
    let contested = contested_roots([("sonarr", sonarr.as_slice()), ("radarr", radarr.as_slice())]);
    assert!(
        contested.is_empty(),
        "no path is shared, so nothing is contested",
    );
}

#[test]
fn a_contested_path_is_recognised_across_a_trailing_slash() {
    // One *arr spells the path with a trailing slash, the other without; they are
    // the same folder, so the clash is not hidden.
    let sonarr = [folder("/data/media/tv/")];
    let radarr = [folder("/data/media/tv")];
    let contested = contested_roots([("sonarr", sonarr.as_slice()), ("radarr", radarr.as_slice())]);
    assert_eq!(contested.len(), 1);
    assert!(contested.contains_key("/data/media/tv"));
}

#[test]
fn one_arr_listing_a_path_twice_does_not_contest_itself() {
    // Distinct services, not repeats, make a contest: one *arr naming a path twice
    // is still one *arr.
    let sonarr = [folder("/data/media/tv"), folder("/data/media/tv")];
    let contested = contested_roots([("sonarr", sonarr.as_slice())]);
    assert!(
        contested.is_empty(),
        "one *arr cannot contest a folder with itself",
    );
}

#[tokio::test]
async fn a_root_folder_outside_the_data_root_is_refused_with_an_explanation() {
    // Every folder lemonfiber builds sits under /data, the mounted data root. One
    // that does not — reached beyond it — is refused, not created: the service
    // would file where its downloads are neither hardlinked to nor visible to the
    // rest of the stack. The refusal names the path and why, and writes nothing.
    let (states, recorded) = seed(
        FakeService::with(Mode::Normal, Vec::new()),
        &[folder("/config/media/tv")],
    )
    .await;
    let reason = match states.as_slice() {
        [State::Refused { reason }] => Some(reason.clone()),
        _ => None,
    };
    assert!(
        reason.is_some_and(|reason| {
            reason.contains("outside the data root") && reason.contains("/config/media/tv")
        }),
        "the refusal names the offending path and why: {states:?}"
    );
    assert_eq!(recorded, 0, "a refused folder writes nothing");
}

#[tokio::test]
async fn an_unavailable_service_skips_every_folder() {
    let (states, recorded) = seed(
        FakeService::with(Mode::Down, Vec::new()),
        &[folder("/data/media/tv"), folder("/data/media/movies")],
    )
    .await;
    assert!(
        states
            .iter()
            .all(|state| matches!(state, State::Skipped { .. })),
        "{states:?}"
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn a_service_that_refuses_the_listing_fails() {
    let (states, _) = seed(
        FakeService::with(Mode::RefusesList, Vec::new()),
        &[folder("/data/media/tv")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Failed { .. }]),
        "{states:?}"
    );
}

#[tokio::test]
async fn a_service_on_an_unsupported_api_version_refuses_its_folders() {
    // The service answers, but not the API version this build speaks. Its folders
    // are refused, not skipped or failed: writing to it would be malformed, and a
    // re-run against the same service cannot lift it — the operator must align the
    // versions. Nothing is written.
    let (states, recorded) = seed(
        FakeService::with(Mode::Unsupported, Vec::new()),
        &[folder("/data/media/tv")],
    )
    .await;
    assert_eq!(
        states,
        vec![State::Refused {
            reason: "sonarr does not serve the API version this build speaks: there is no /api/v3"
                .to_owned(),
        }]
    );
    assert_eq!(recorded, 0, "nothing is written to an unsupported version");
}

#[tokio::test]
async fn a_rejected_registration_fails_with_the_services_own_words() {
    let (states, recorded) = seed(
        FakeService::with(Mode::RejectsRegister, Vec::new()),
        &[folder("/data/media/tv")],
    )
    .await;
    let detail = match states.as_slice() {
        [State::Failed { detail }] => Some(detail.clone()),
        _ => None,
    };
    assert!(
        detail.is_some_and(|words| words.contains("already used")),
        "the service's own words survive: {states:?}"
    );
    assert_eq!(recorded, 0, "a rejected write is not journalled");
}

#[tokio::test]
async fn a_write_that_does_not_appear_when_read_back_is_a_failure() {
    // The service accepted the registration but does not report the folder, so it
    // did not land — not done, and not recorded.
    let (states, recorded) = seed(
        FakeService::with(Mode::Swallows, Vec::new()),
        &[folder("/data/media/tv")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Failed { .. }]),
        "{states:?}"
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn a_service_that_stops_answering_after_the_write_is_skipped() {
    // The write went out but could not be confirmed, so it is left for a later
    // run to reconcile rather than declared wired.
    let (states, recorded) = seed(
        FakeService::with(Mode::DropsAfterRegister, Vec::new()),
        &[folder("/data/media/tv")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Skipped { .. }]),
        "{states:?}"
    );
    assert_eq!(recorded, 0, "an unconfirmed write is not recorded as done");
}

// ---- Download clients: the same driver, matched by endpoint not label. ----

fn client(name: &str, host: &str, port: u16) -> DownloadClient {
    client_with_category(name, host, port, "tv")
}

/// A wanted client whose category lemonfiber intends to file under `category` —
/// for the drift tests, where lemonfiber's desired value is the thing that moves.
fn client_with_category(name: &str, host: &str, port: u16, category: &str) -> DownloadClient {
    DownloadClient {
        name: name.to_owned(),
        host: host.to_owned(),
        port,
        kind: ClientKind::Sabnzbd,
        credential: Credential::ApiKey("sab-key".to_owned()),
        category: Category {
            field: "tvCategory".to_owned(),
            value: category.to_owned(),
        },
    }
}

/// Run the client driver for the wanted clients, returning their resulting states
/// and the number of changes journalled. The baseline it records into is discarded
/// — the tests that assert on it drive the driver directly.
async fn seed_clients(service: FakeService, wanted: &[DownloadClient]) -> (Vec<State>, usize) {
    let mut journal = Journal::new();
    let expected = Baseline::new();
    let mut records = Baseline::new();
    let wirings = wire_download_clients(
        &service,
        "sonarr",
        wanted,
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: false,
        },
        "t",
    )
    .await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, journal.changes().len())
}

/// Run the client driver, returning the resulting states and the baseline it
/// recorded into — for the tests that assert what lemonfiber remembered it wrote.
/// The expected snapshot is empty, as on a first seed.
async fn seed_clients_recording(
    service: FakeService,
    wanted: &[DownloadClient],
) -> (Vec<State>, Baseline) {
    let mut journal = Journal::new();
    let expected = Baseline::new();
    let mut records = Baseline::new();
    let wirings = wire_download_clients(
        &service,
        "sonarr",
        wanted,
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: false,
        },
        "t",
    )
    .await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, records)
}

#[tokio::test]
async fn an_absent_download_client_is_registered_read_back_and_recorded() {
    let (states, recorded) = seed_clients(
        FakeService::with_clients(Mode::Normal, Vec::new()),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    assert_eq!(states, vec![State::Wired]);
    assert_eq!(recorded, 1, "the write is journalled so it can be undone");
}

#[tokio::test]
async fn a_client_at_the_same_endpoint_is_left_untouched_despite_a_different_name() {
    // The connection detail, not the label, decides identity: the operator
    // renamed the client, but it reaches the same host and port, so it is left
    // alone rather than registered a second time.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "qbittorrent".to_owned(),
        port: 8080,
        // Same category as wanted, so only the name differs.
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        }),
    }];
    let (states, recorded) = seed_clients(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("qBittorrent — my own name", "qbittorrent", 8080)],
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(
        recorded, 0,
        "a client already at the endpoint is not duplicated"
    );
}

#[tokio::test]
async fn a_client_the_operator_re_filed_is_preserved_as_drift() {
    // lemonfiber last wrote "tv" and still wants "tv", but the operator changed the
    // category in the *arr itself. That is their edit to keep, not a mistake to
    // revert: against the baseline it reports as drift and is left exactly as it is,
    // nothing re-registered.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "qbittorrent".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "my-own-tv".to_owned(),
        }),
    }];
    let mut journal = Journal::new();
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:qbittorrent:8080", "tv", "1");
    let mut records = Baseline::new();
    let states: Vec<State> = wire_download_clients(
        &FakeService::with_clients(Mode::Normal, existing),
        "sonarr",
        &[client("qBittorrent", "qbittorrent", 8080)],
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: false,
        },
        "2",
    )
    .await
    .into_iter()
    .map(|wiring| wiring.state)
    .collect();
    assert_eq!(states, vec![State::Drifted]);
    assert_eq!(
        journal.changes().len(),
        0,
        "an operator's own change is preserved, not rewritten"
    );
}

/// A qBittorrent client the operator re-filed, so it reads as drift — the setup a
/// severity check reads. Recorded "tv", the service now holds "my-own-tv".
fn re_filed_client() -> Vec<RegisteredClient> {
    vec![RegisteredClient {
        id: "1".to_owned(),
        host: "qbittorrent".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "my-own-tv".to_owned(),
        }),
    }]
}

/// Wire the wanted clients against a service holding `existing`, with lemonfiber's own
/// value recorded for the qBittorrent endpoint so a differing one reads as drift, and
/// the given test verdicts — `None` to stand in for a service that will not test.
async fn seed_clients_probed(
    existing: Vec<RegisteredClient>,
    wanted: &[DownloadClient],
    probes: Option<Vec<ClientProbe>>,
) -> Vec<Wiring> {
    let mut journal = Journal::new();
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:qbittorrent:8080", "tv", "1");
    let mut records = Baseline::new();
    let base = FakeService::with_clients(Mode::Normal, existing);
    let service = match probes {
        Some(probes) => base.probing(probes),
        None => base,
    };
    wire_download_clients(
        &service,
        "sonarr",
        wanted,
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: false,
        },
        "2",
    )
    .await
}

/// One test verdict for a client, by id.
fn probe(id: &str, reachable: bool, detail: Option<&str>) -> ClientProbe {
    ClientProbe {
        id: id.to_owned(),
        reachable,
        detail: detail.map(str::to_owned),
    }
}

/// The breakage a wiring's warning names, or nothing where it is informational or
/// absent — the one place a severity is read, so both arms are exercised across the
/// warning and the informational tests rather than left dead in either.
fn breakage(wiring: Option<&Wiring>) -> Option<String> {
    match wiring.map(|wiring| &wiring.severity) {
        Some(Severity::Warning { breakage, .. }) => Some(breakage.clone()),
        Some(Severity::Informational) | None => None,
    }
}

/// The single wanted qBittorrent client the drift-severity tests re-file.
fn one_qbittorrent() -> [DownloadClient; 1] {
    [client("qBittorrent", "qbittorrent", 8080)]
}

#[tokio::test]
async fn a_drifted_client_the_service_cannot_reach_is_raised_to_a_warning() {
    // A category drift is the operator's own edit, ordinarily just information. But
    // the same client the service can no longer reach has broken the stack — nothing
    // downloads through it — so it is raised to a warning naming the service's own
    // words. A second, freshly-wired client alongside it is not a drift, so it is
    // never tested and stays informational.
    let wanted = [
        client("qBittorrent", "qbittorrent", 8080),
        client("SABnzbd", "sabnzbd", 8080),
    ];
    let wirings = seed_clients_probed(
        re_filed_client(),
        &wanted,
        Some(vec![probe("1", false, Some("connection refused"))]),
    )
    .await;

    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&State::Drifted)
    );
    assert!(
        breakage(wirings.first()).is_some_and(|breakage| breakage.contains("connection refused")),
        "the unreachable drift is a warning naming the service's words"
    );
    // The freshly-wired SABnzbd client never drifted, so it was never tested.
    assert_eq!(
        wirings.get(1).map(|wiring| &wiring.state),
        Some(&State::Wired)
    );
    assert!(breakage(wirings.get(1)).is_none());
}

#[tokio::test]
async fn a_drifted_client_the_service_still_reaches_stays_informational() {
    // A drift the service can still reach has broken nothing — it is the operator's
    // edit, working — so it is left as the information it is.
    let wirings = seed_clients_probed(
        re_filed_client(),
        &one_qbittorrent(),
        Some(vec![probe("1", true, None)]),
    )
    .await;
    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&State::Drifted)
    );
    assert!(breakage(wirings.first()).is_none());
}

#[tokio::test]
async fn an_unreachable_client_the_service_gave_no_words_for_names_a_fallback() {
    // A test that failed without the service saying why still names the breakage, so
    // the warning is never blank — a fallback stands in for the missing detail.
    let wirings = seed_clients_probed(
        re_filed_client(),
        &one_qbittorrent(),
        Some(vec![probe("1", false, None)]),
    )
    .await;
    assert!(
        breakage(wirings.first()).is_some_and(|breakage| breakage.contains("could not reach it")),
        "the warning names a fallback where the service gave no words"
    );
}

#[tokio::test]
async fn a_drift_the_service_will_not_test_stays_the_information_it_is() {
    // A service that will not run the test at all proves nothing broken, so the drift
    // is left as information rather than guessed into a warning.
    let wirings = seed_clients_probed(re_filed_client(), &one_qbittorrent(), None).await;
    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&State::Drifted)
    );
    assert!(breakage(wirings.first()).is_none());
}

#[tokio::test]
async fn a_drift_the_test_does_not_cover_stays_informational() {
    // The service tested its clients but reported nothing for this one — no verdict is
    // not a failure, so the drift stays the information it is.
    let wirings = seed_clients_probed(
        re_filed_client(),
        &one_qbittorrent(),
        Some(vec![probe("999", false, Some("some other client"))]),
    )
    .await;
    assert!(breakage(wirings.first()).is_none());
}

/// A client the service holds under a category, for the wholesale-drift checks.
fn holding(id: &str, host: &str, port: u16, category: &str) -> RegisteredClient {
    RegisteredClient {
        id: id.to_owned(),
        host: host.to_owned(),
        port,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: category.to_owned(),
        }),
    }
}

#[test]
fn every_client_drifted_at_once_reads_as_wholesale() {
    // lemonfiber recorded "tv"; the one client the service holds now reads "shows".
    // With every managed value moved together, this is a schema change, not the
    // operator editing each by hand.
    let existing = vec![holding("1", "qbittorrent", 8080, "shows")];
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:qbittorrent:8080", "tv", "1");
    let wanted = [client("qBittorrent", "qbittorrent", 8080)];
    assert!(wholesale_drift(&existing, &wanted, &expected, "sonarr"));
}

#[test]
fn one_client_still_at_lemonfibers_value_is_not_wholesale() {
    // Two clients the service holds: one drifted, one still at lemonfiber's value. Not
    // every managed value moved, so it is the operator's edits — reported as drift, not
    // re-baselined.
    let existing = vec![
        holding("1", "qbittorrent", 8080, "shows"),
        holding("2", "sabnzbd", 8080, "tv"),
    ];
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:qbittorrent:8080", "tv", "1");
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let wanted = [
        client("qBittorrent", "qbittorrent", 8080),
        client("SABnzbd", "sabnzbd", 8080),
    ];
    assert!(!wholesale_drift(&existing, &wanted, &expected, "sonarr"));
}

#[test]
fn a_service_holding_none_of_the_wanted_clients_is_not_wholesale() {
    // Nothing present drifted, so there is no wholesale drift to read — a client not
    // there yet does not, on its own, stand in for a schema change.
    let wanted = [client("qBittorrent", "qbittorrent", 8080)];
    assert!(!wholesale_drift(&[], &wanted, &Baseline::new(), "sonarr"));
}

#[tokio::test]
async fn a_wired_client_records_its_category_as_the_expected_baseline() {
    // What lemonfiber writes it remembers: the category is recorded, keyed by the
    // client's endpoint, so a later run can tell an operator's re-filing from
    // lemonfiber's own value.
    let (states, baseline) = seed_clients_recording(
        FakeService::with_clients(Mode::Normal, Vec::new()),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    assert_eq!(states, vec![State::Wired]);
    assert_eq!(
        baseline.expected("sonarr", "downloadclient:sabnzbd:8080"),
        Some("tv"),
    );
}

#[tokio::test]
async fn a_client_already_at_lemonfibers_value_is_recorded_as_the_baseline_too() {
    // An already-correct client was not written this run, but it is lemonfiber's
    // value, so it is recorded as expected — which is also how a lost baseline
    // re-forms from what already matches lemonfiber's intent.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        }),
    }];
    let (states, baseline) = seed_clients_recording(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(
        baseline.expected("sonarr", "downloadclient:sabnzbd:8080"),
        Some("tv"),
    );
}

#[tokio::test]
async fn an_operators_re_filed_client_is_not_recorded_as_the_baseline() {
    // A drifted client is the operator's edit, not lemonfiber's value, so its
    // category is not recorded as expected: the baseline keeps what lemonfiber last
    // wrote — here "tv" — which is what lets a later run read the difference as
    // drift rather than as lemonfiber's own intent.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "my-own-tv".to_owned(),
        }),
    }];
    let mut journal = Journal::new();
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let mut records = Baseline::new();
    let states: Vec<State> = wire_download_clients(
        &FakeService::with_clients(Mode::Normal, existing),
        "sonarr",
        &[client("SABnzbd", "sabnzbd", 8080)],
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: false,
        },
        "2",
    )
    .await
    .into_iter()
    .map(|wiring| wiring.state)
    .collect();
    let baseline = records;
    assert_eq!(states, vec![State::Drifted]);
    assert_eq!(
        baseline.expected("sonarr", "downloadclient:sabnzbd:8080"),
        None,
    );
}

#[tokio::test]
async fn a_reset_reverts_a_drifted_category_to_lemonfibers() {
    // The same drift as above — the operator changed the category — but a reset writes
    // lemonfiber's own back over it: the client is updated in place, the connection reads
    // wired again, and the reverted value is recorded so the drift is gone.
    let service = FakeService::with_clients(
        Mode::Normal,
        vec![RegisteredClient {
            id: "1".to_owned(),
            host: "sabnzbd".to_owned(),
            port: 8080,
            category: Some(Category {
                field: "tvCategory".to_owned(),
                value: "my-own-tv".to_owned(),
            }),
        }],
    );
    let mut journal = Journal::new();
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let mut records = Baseline::new();
    let states: Vec<State> = wire_download_clients(
        &service,
        "sonarr",
        &[client("SABnzbd", "sabnzbd", 8080)],
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: true,
        },
        "2",
    )
    .await
    .into_iter()
    .map(|wiring| wiring.state)
    .collect();
    assert_eq!(states, vec![State::Wired], "the drift is reverted to wired");
    // lemonfiber's value is recorded, so a later run reads no drift.
    assert_eq!(
        records.expected("sonarr", "downloadclient:sabnzbd:8080"),
        Some("tv"),
    );
}

#[tokio::test]
async fn a_reset_a_service_refuses_is_reported_as_failed_not_recorded() {
    // A reset whose in-place update the service will not take leaves the drift reported
    // as a failure rather than falsely recorded as reverted.
    let service = FakeService::with_clients(
        Mode::RefusesUpdate,
        vec![RegisteredClient {
            id: "1".to_owned(),
            host: "sabnzbd".to_owned(),
            port: 8080,
            category: Some(Category {
                field: "tvCategory".to_owned(),
                value: "my-own-tv".to_owned(),
            }),
        }],
    );
    let mut journal = Journal::new();
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let mut records = Baseline::new();
    let wirings = wire_download_clients(
        &service,
        "sonarr",
        &[client("SABnzbd", "sabnzbd", 8080)],
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: true,
        },
        "2",
    )
    .await;
    assert!(matches!(
        wirings.first().map(|wiring| &wiring.state),
        Some(State::Failed { .. })
    ));
    assert_eq!(
        records.expected("sonarr", "downloadclient:sabnzbd:8080"),
        None
    );
}

#[tokio::test]
async fn a_reset_registers_nothing_a_preview_did_not_show() {
    // A reset reverts drift and only drift. A client the service does not hold — never
    // registered, so absent — is not a drift to revert, so a reset leaves it: it is not
    // registered, not reported as wired, and not recorded. A confirmed reset must do no
    // more than its preview showed, which listed no absent connection.
    let service = FakeService::with_clients(Mode::Normal, Vec::new());
    let mut journal = Journal::new();
    let mut records = Baseline::new();
    let wirings = wire_download_clients(
        &service,
        "sonarr",
        &[client("SABnzbd", "sabnzbd", 8080)],
        &mut journal,
        &mut Baselines {
            expected: &Baseline::new(),
            records: &mut records,
            adopt: false,
            reset: true,
        },
        "2",
    )
    .await;
    assert!(
        !wirings
            .iter()
            .any(|wiring| matches!(wiring.state, State::Wired)),
        "an absent client is never registered by a reset"
    );
    assert_eq!(
        journal.changes().len(),
        0,
        "a reset writes nothing for a client that was never there"
    );
    assert_eq!(
        records.expected("sonarr", "downloadclient:sabnzbd:8080"),
        None,
        "a reset records nothing for a client it did not touch"
    );
}

#[tokio::test]
async fn a_categoryless_client_lemonfiber_never_wrote_is_left_and_not_recorded() {
    // The service holds a client at the endpoint but reports no category, and there
    // is no baseline — lemonfiber never wrote it. With nothing to judge against it is
    // the operator's own, pre-existing and unmanaged, left as it is; and with no
    // value to adopt (the client is categoryless) nothing is recorded, so a later run
    // reads the operator's eventual category as their own value, not a conflict
    // against a baseline lemonfiber never set.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: None,
    }];
    let (states, baseline) = seed_clients_recording(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    assert_eq!(states, vec![State::Unmanaged]);
    assert_eq!(
        baseline.expected("sonarr", "downloadclient:sabnzbd:8080"),
        None,
    );
}

#[tokio::test]
async fn a_client_at_lemonfibers_old_value_with_a_moved_intent_is_stale() {
    // The baseline records lemonfiber last wrote "tv"; the service still holds "tv",
    // but lemonfiber now wants "tv-hd". Only lemonfiber's intent moved, so the
    // client is lemonfiber's own value fallen behind — stale, left as it is (never
    // overwritten) and reported, not preserved as though it were an operator edit.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        }),
    }];
    let mut journal = Journal::new();
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let mut records = Baseline::new();
    let states: Vec<State> = wire_download_clients(
        &FakeService::with_clients(Mode::Normal, existing),
        "sonarr",
        &[client_with_category("SABnzbd", "sabnzbd", 8080, "tv-hd")],
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: false,
        },
        "2",
    )
    .await
    .into_iter()
    .map(|wiring| wiring.state)
    .collect();
    assert_eq!(states, vec![State::Stale]);
    assert_eq!(
        journal.changes().len(),
        0,
        "a stale value is not overwritten"
    );
}

#[tokio::test]
async fn a_client_both_sides_changed_is_a_conflict() {
    // The baseline records "tv"; the operator re-filed to "mine" and lemonfiber now
    // wants "tv-hd". Both moved away from the baseline, so lemonfiber presents the
    // conflict and leaves the value — it does not resolve it on its own.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "mine".to_owned(),
        }),
    }];
    let mut journal = Journal::new();
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let mut records = Baseline::new();
    let states: Vec<State> = wire_download_clients(
        &FakeService::with_clients(Mode::Normal, existing),
        "sonarr",
        &[client_with_category("SABnzbd", "sabnzbd", 8080, "tv-hd")],
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: false,
        },
        "2",
    )
    .await
    .into_iter()
    .map(|wiring| wiring.state)
    .collect();
    // The conflict is presented with both sides — what the operator set beside what
    // lemonfiber would write — so the operator can see the clash, and nothing is
    // written: presenting is not resolving.
    assert_eq!(
        states,
        vec![State::Conflicted {
            yours: Some("mine".to_owned()),
            ours: "tv-hd".to_owned(),
        }]
    );
    assert_eq!(journal.changes().len(), 0, "a conflict is not resolved");
}

#[tokio::test]
async fn a_category_differing_only_by_whitespace_is_not_drift() {
    // lemonfiber wrote "tv" and still wants it; the service reports it back with
    // surrounding whitespace lemonfiber's own value does not carry — the kind of
    // difference a normalisation on write leaves. Compared by canonical form the two
    // are the same category, so it reads as already wired, not as drift to preserve.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: " tv ".to_owned(),
        }),
    }];
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let (states, _recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        false,
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(
        changes, 0,
        "a value the same but for whitespace is not written again"
    );
}

#[tokio::test]
async fn a_whitespace_only_difference_with_no_baseline_is_lemonfibers_own() {
    // No baseline, and the service holds what lemonfiber would write but for
    // surrounding whitespace. Canonically the two are the same, so this is not the
    // operator's own unmanaged value — it is lemonfiber's, already in place: already
    // wired, and recorded as written, not adopted.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: " tv ".to_owned(),
        }),
    }];
    let (states, recorded, _changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &Baseline::new(),
        false,
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(
        recorded.entry("sonarr", "downloadclient:sabnzbd:8080"),
        Some(&Record {
            value: "tv".to_owned(),
            at: "2".to_owned(),
            origin: Origin::Written,
        }),
        "the value is lemonfiber's own, recorded as written not adopted",
    );
}

/// Run the client driver against a given baseline and pass kind, returning the
/// resulting states, what this run recorded, and the number of changes journalled —
/// for the adoption tests, which turn on the baseline's origin and the adopt flag.
async fn seed_clients_with(
    service: FakeService,
    wanted: &[DownloadClient],
    expected: &Baseline,
    adopt: bool,
) -> (Vec<State>, Baseline, usize) {
    let mut journal = Journal::new();
    let mut records = Baseline::new();
    let wirings = wire_download_clients(
        &service,
        "sonarr",
        wanted,
        &mut journal,
        &mut Baselines {
            expected,
            records: &mut records,
            adopt,
            reset: false,
        },
        "2",
    )
    .await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, records, journal.changes().len())
}

fn a_pre_existing_client() -> Vec<RegisteredClient> {
    vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "their-tv".to_owned(),
        }),
    }]
}

#[tokio::test]
async fn a_pre_existing_value_with_no_baseline_is_reported_unmanaged_not_drift() {
    // A service already configured before lemonfiber managed it: it holds a value
    // lemonfiber never wrote, and there is no baseline. An ordinary seed reports it
    // as unmanaged — the operator's own, outside lemonfiber's scope — rather than as
    // mass drift, and records nothing: it does not claim the value as lemonfiber's on
    // its own, so a lost baseline is never silently frozen.
    let (states, recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, a_pre_existing_client()),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &Baseline::new(),
        false,
    )
    .await;
    assert_eq!(states, vec![State::Unmanaged]);
    assert_eq!(
        changes, 0,
        "an unmanaged value is not written to the service"
    );
    assert_eq!(
        recorded.entry("sonarr", "downloadclient:sabnzbd:8080"),
        None,
        "an ordinary seed does not adopt a pre-existing value on its own",
    );
}

#[tokio::test]
async fn an_adopt_pass_takes_on_a_pre_existing_unmanaged_value() {
    // The deliberate act: adopting an existing setup baselines from what is found.
    // The same pre-existing value, run through an adopt pass, is taken on — recorded
    // as the operator's own, marked adopted, with nothing written to the service.
    let (states, recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, a_pre_existing_client()),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &Baseline::new(),
        true,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(
        changes, 0,
        "adopting what is found writes nothing to the service"
    );
    assert_eq!(
        recorded.entry("sonarr", "downloadclient:sabnzbd:8080"),
        Some(&Record {
            value: "their-tv".to_owned(),
            at: "2".to_owned(),
            origin: Origin::Adopted,
        }),
        "the operator's own value is adopted as the baseline",
    );
}

#[tokio::test]
async fn an_adopted_value_lemonfiber_also_wants_stays_adopted_not_re_recorded() {
    // The case the origin exists to guard: an adopted value that happens to equal what
    // lemonfiber would write must stay adopted, not be read as merely in-sync and taken
    // back as lemonfiber's own. It reads as adopted, and this run records nothing over
    // it — so the adopted baseline is not clobbered with a written one, which a later
    // run, once lemonfiber's desired moved, would read as stale and revert.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        }),
    }];
    let mut expected = Baseline::new();
    expected.adopt("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let (states, recorded, _changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        false,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(
        recorded.entry("sonarr", "downloadclient:sabnzbd:8080"),
        None,
        "an adopted value equal to desired is not re-recorded as written",
    );
}

#[tokio::test]
async fn an_adopted_value_the_service_still_holds_is_kept_not_made_stale() {
    // The run after adoption: the baseline now holds the operator's value, marked
    // adopted, and the service still holds it, while lemonfiber's desired differs. A
    // written value here would be stale — lemonfiber's own, to bring up to date — but
    // an adopted one is theirs to keep, so it is left as it is and not overwritten.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "their-tv".to_owned(),
        }),
    }];
    let mut expected = Baseline::new();
    expected.adopt("sonarr", "downloadclient:sabnzbd:8080", "their-tv", "1");
    let (states, _recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        false,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(changes, 0, "an adopted value is not overwritten");
}

#[tokio::test]
async fn an_adopt_pass_promotes_a_drifted_value_and_records_it_as_adopted() {
    // lemonfiber wrote "tv" and still wants it, but the operator changed it: a normal
    // seed reports drift. An adopt pass instead promotes their edit — reporting it
    // adopted and recording what the service holds as the accepted baseline, so a
    // later seed keeps it rather than flagging it again.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "my-own-tv".to_owned(),
        }),
    }];
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let (states, recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        true,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(
        changes, 0,
        "adopting an edit rewrites nothing in the service"
    );
    assert_eq!(
        recorded.entry("sonarr", "downloadclient:sabnzbd:8080"),
        Some(&Record {
            value: "my-own-tv".to_owned(),
            at: "2".to_owned(),
            origin: Origin::Adopted,
        }),
        "the operator's edit is recorded as the adopted baseline",
    );
}

#[tokio::test]
async fn a_seed_after_an_adopt_pass_keeps_the_adopted_edit() {
    // What the adopt pass recorded above, read on the next ordinary seed: the value
    // is adopted, so the seed keeps it rather than reverting to lemonfiber's default —
    // the promotion survives future seeds.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "my-own-tv".to_owned(),
        }),
    }];
    let mut expected = Baseline::new();
    expected.adopt("sonarr", "downloadclient:sabnzbd:8080", "my-own-tv", "2");
    let (states, _recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        false,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(changes, 0, "the adopted edit is kept, not reverted");
}

#[tokio::test]
async fn an_unavailable_service_skips_every_download_client() {
    let (states, recorded) = seed_clients(
        FakeService::with_clients(Mode::Down, Vec::new()),
        &[
            client("SABnzbd", "sabnzbd", 8080),
            client("qBittorrent", "qbittorrent", 8080),
        ],
    )
    .await;
    assert!(
        states
            .iter()
            .all(|state| matches!(state, State::Skipped { .. })),
        "{states:?}"
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn a_service_that_refuses_the_client_listing_fails() {
    let (states, _) = seed_clients(
        FakeService::with_clients(Mode::RefusesList, Vec::new()),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Failed { .. }]),
        "{states:?}"
    );
}

#[tokio::test]
async fn a_rejected_client_registration_fails_with_the_services_own_words() {
    let (states, recorded) = seed_clients(
        FakeService::with_clients(Mode::RejectsRegister, Vec::new()),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    let detail = match states.as_slice() {
        [State::Failed { detail }] => Some(detail.clone()),
        _ => None,
    };
    assert!(
        detail.is_some_and(|words| words.contains("unknown implementation")),
        "the service's own words survive: {states:?}"
    );
    assert_eq!(recorded, 0, "a rejected write is not journalled");
}

#[tokio::test]
async fn a_client_write_that_does_not_appear_when_read_back_is_a_failure() {
    // Accepted but not reported back, so it did not land — not done, not recorded.
    let (states, recorded) = seed_clients(
        FakeService::with_clients(Mode::Swallows, Vec::new()),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Failed { .. }]),
        "{states:?}"
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn a_service_that_stops_answering_after_the_client_write_is_skipped() {
    // The write went out but could not be confirmed, so it is left for a later
    // run to reconcile rather than declared wired.
    let (states, recorded) = seed_clients(
        FakeService::with_clients(Mode::DropsAfterRegister, Vec::new()),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Skipped { .. }]),
        "{states:?}"
    );
    assert_eq!(recorded, 0, "an unconfirmed write is not recorded as done");
}

// ---- Prowlarr applications: the same driver, matched by base URL not label. ----

/// A Prowlarr that answers the app-sync driver from a script.
struct FakeProwlarr {
    mode: Mode,
    applications: Mutex<Vec<RegisteredApplication>>,
    reads: Mutex<u32>,
    next_id: Mutex<u32>,
}

impl FakeProwlarr {
    fn with(mode: Mode, applications: Vec<RegisteredApplication>) -> Self {
        Self {
            mode,
            applications: Mutex::new(applications),
            reads: Mutex::new(0),
            next_id: Mutex::new(100),
        }
    }
}

#[async_trait]
impl AppSync for FakeProwlarr {
    async fn register_application(&self, application: &Application) -> Result<(), Failure> {
        match self.mode {
            Mode::Down => Err(down("prowlarr")),
            Mode::RejectsRegister => Err(Failure::Refused {
                service: "prowlarr".to_owned(),
                detail: "HTTP 400: unknown implementation".to_owned(),
            }),
            Mode::Swallows => Ok(()),
            _ => {
                if let (Ok(mut applications), Ok(mut id)) =
                    (self.applications.lock(), self.next_id.lock())
                {
                    applications.push(RegisteredApplication {
                        id: id.to_string(),
                        // Prowlarr stores the canonical address, dropping a
                        // trailing slash — as its `fields` read back.
                        base_url: application.base_url.trim_end_matches('/').to_owned(),
                    });
                    *id += 1;
                }
                Ok(())
            }
        }
    }

    async fn applications(&self) -> Result<Vec<RegisteredApplication>, Failure> {
        let count = match self.reads.lock() {
            Ok(mut reads) => {
                *reads += 1;
                *reads
            }
            Err(_) => 0,
        };
        match self.mode {
            Mode::Down => Err(down("prowlarr")),
            Mode::RefusesList => Err(Failure::Unauthorised {
                service: "prowlarr".to_owned(),
            }),
            Mode::DropsAfterRegister if count >= 2 => Err(down("prowlarr")),
            _ => Ok(self
                .applications
                .lock()
                .map(|applications| applications.clone())
                .unwrap_or_default()),
        }
    }
}

fn app(base_url: &str) -> Application {
    Application {
        name: "Sonarr".to_owned(),
        kind: ApplicationKind::Sonarr,
        prowlarr_url: "http://prowlarr:9696".to_owned(),
        base_url: base_url.to_owned(),
        api_key: "arr-key".to_owned(),
    }
}

/// Run the app-sync driver for the wanted applications, returning their states
/// and the number of changes journalled.
async fn seed_applications(prowlarr: FakeProwlarr, wanted: &[Application]) -> (Vec<State>, usize) {
    let mut journal = Journal::new();
    let wirings = wire_applications(&prowlarr, "prowlarr", wanted, &mut journal, "t").await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, journal.changes().len())
}

#[tokio::test]
async fn an_absent_application_is_registered_read_back_and_recorded() {
    let (states, recorded) = seed_applications(
        FakeProwlarr::with(Mode::Normal, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    assert_eq!(states, vec![State::Wired]);
    assert_eq!(recorded, 1, "the write is journalled so it can be undone");
}

#[tokio::test]
async fn an_application_at_the_same_base_url_is_left_untouched_despite_a_different_name() {
    // Identity is the address Prowlarr reaches, not the label: the operator
    // renamed the application, but it points at the same *arr, so it is left
    // alone rather than registered a second time.
    let existing = vec![RegisteredApplication {
        id: "1".to_owned(),
        base_url: "http://sonarr:8989".to_owned(),
    }];
    let mut renamed = app("http://sonarr:8989");
    renamed.name = "Sonarr — my own name".to_owned();
    let (states, recorded) =
        seed_applications(FakeProwlarr::with(Mode::Normal, existing), &[renamed]).await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(
        recorded, 0,
        "an application already at the address is not duplicated"
    );
}

#[tokio::test]
async fn a_present_application_is_matched_despite_a_trailing_slash() {
    // Idempotent across the same normalization: Prowlarr holds the canonical
    // address, and a wanted one that differs only by a trailing slash is left
    // alone, not re-registered.
    let existing = vec![RegisteredApplication {
        id: "1".to_owned(),
        base_url: "http://sonarr:8989".to_owned(),
    }];
    let (states, recorded) = seed_applications(
        FakeProwlarr::with(Mode::Normal, existing),
        &[app("http://sonarr:8989/")],
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn an_unavailable_prowlarr_skips_every_application() {
    let (states, recorded) = seed_applications(
        FakeProwlarr::with(Mode::Down, Vec::new()),
        &[app("http://sonarr:8989"), app("http://radarr:7878")],
    )
    .await;
    assert!(
        states
            .iter()
            .all(|state| matches!(state, State::Skipped { .. })),
        "{states:?}"
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn a_prowlarr_that_refuses_the_application_listing_fails() {
    let (states, _) = seed_applications(
        FakeProwlarr::with(Mode::RefusesList, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Failed { .. }]),
        "{states:?}"
    );
}

#[tokio::test]
async fn a_rejected_application_registration_fails_with_prowlarrs_own_words() {
    let (states, recorded) = seed_applications(
        FakeProwlarr::with(Mode::RejectsRegister, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    let detail = match states.as_slice() {
        [State::Failed { detail }] => Some(detail.clone()),
        _ => None,
    };
    assert!(
        detail.is_some_and(|words| words.contains("unknown implementation")),
        "the service's own words survive: {states:?}"
    );
    assert_eq!(recorded, 0, "a rejected write is not journalled");
}

#[tokio::test]
async fn an_application_write_that_does_not_appear_when_read_back_is_a_failure() {
    // Accepted but not reported back, so it did not land — not done, not recorded.
    let (states, recorded) = seed_applications(
        FakeProwlarr::with(Mode::Swallows, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Failed { .. }]),
        "{states:?}"
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn a_prowlarr_that_stops_answering_after_the_write_is_skipped() {
    // The write went out but could not be confirmed, so it is left for a later
    // run to reconcile rather than declared wired.
    let (states, recorded) = seed_applications(
        FakeProwlarr::with(Mode::DropsAfterRegister, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Skipped { .. }]),
        "{states:?}"
    );
    assert_eq!(recorded, 0, "an unconfirmed write is not recorded as done");
}

// ---- Jellyfin as Seerr's identity: two services and a minted credential. ----

/// How Jellyfin's setup answers.
enum Startup {
    /// The wizard has already run.
    Completed,
    /// The wizard has not run.
    Fresh,
    /// Not answering.
    Down,
}

/// How Jellyfin answers the account creation. A refusal stands for every
/// non-success; the not-answering path is exercised through the startup read.
enum Create {
    Ok,
    Rejects,
}

struct FakeMedia {
    startup: Startup,
    create: Create,
}

#[async_trait]
impl MediaServer for FakeMedia {
    async fn startup_completed(&self) -> Result<bool, Failure> {
        match self.startup {
            Startup::Completed => Ok(true),
            Startup::Fresh => Ok(false),
            Startup::Down => Err(down("jellyfin")),
        }
    }

    async fn create_admin(&self, _name: &str, _password: &str) -> Result<(), Failure> {
        match self.create {
            Create::Ok => Ok(()),
            Create::Rejects => Err(Failure::Refused {
                service: "jellyfin".to_owned(),
                detail: "HTTP 400: user already exists".to_owned(),
            }),
        }
    }
}

/// How Seerr answers a read of its initialised state.
enum Init {
    /// Not initialised.
    Fresh,
    /// Already initialised.
    Done,
    /// Not answering.
    Down,
}

/// How Seerr answers the sign-in. A refusal stands for every non-success; the
/// not-answering path is exercised through the initialised read.
enum Configure {
    Ok,
    Rejects,
}

struct FakeReq {
    /// The state reported on the first read (the gate) and on the second (the
    /// read-back), so a fresh-then-confirmed sequence can be scripted.
    gate: Init,
    readback: Init,
    configure: Configure,
    calls: Mutex<u32>,
}

impl FakeReq {
    fn new(gate: Init, readback: Init, configure: Configure) -> Self {
        Self {
            gate,
            readback,
            configure,
            calls: Mutex::new(0),
        }
    }

    fn reading(state: &Init) -> Result<bool, Failure> {
        match state {
            Init::Fresh => Ok(false),
            Init::Done => Ok(true),
            Init::Down => Err(down("seerr")),
        }
    }
}

#[async_trait]
impl Requests for FakeReq {
    async fn initialized(&self) -> Result<bool, Failure> {
        let count = match self.calls.lock() {
            Ok(mut calls) => {
                *calls += 1;
                *calls
            }
            Err(_) => 0,
        };
        if count <= 1 {
            Self::reading(&self.gate)
        } else {
            Self::reading(&self.readback)
        }
    }

    async fn configure_identity(
        &self,
        _username: &str,
        _password: &str,
        _server_url: &str,
    ) -> Result<(), Failure> {
        match self.configure {
            Configure::Ok => Ok(()),
            Configure::Rejects => Err(Failure::Refused {
                service: "seerr".to_owned(),
                detail: "HTTP 500: credentials rejected".to_owned(),
            }),
        }
    }
}

struct FixedRandom(Option<Vec<u8>>);

impl Random for FixedRandom {
    fn bytes(&self, _n: usize) -> Option<Vec<u8>> {
        self.0.clone()
    }
}

/// Run the identity driver, returning the resulting state and the password to
/// record (present only when the account was newly minted).
async fn identity(
    media: FakeMedia,
    seerr: FakeReq,
    random: Option<Vec<u8>>,
    recorded: Option<&str>,
) -> (State, Option<String>) {
    let random = FixedRandom(random);
    let (wiring, minted) =
        wire_jellyfin_identity(&media, &seerr, &random, recorded, "http://jellyfin:8096").await;
    (wiring.state, minted)
}

fn media(startup: Startup, create: Create) -> FakeMedia {
    FakeMedia { startup, create }
}

const RANDOM: [u8; 24] = [0x11; 24];

#[tokio::test]
async fn a_jellyfin_that_is_not_answering_skips_the_identity() {
    let (state, minted) = identity(
        media(Startup::Down, Create::Ok),
        FakeReq::new(Init::Fresh, Init::Done, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Skipped { .. }), "{state:?}");
    assert!(minted.is_none(), "nothing was minted");
}

#[tokio::test]
async fn no_randomness_fails_rather_than_setting_a_guessable_password() {
    let (state, minted) = identity(
        media(Startup::Fresh, Create::Ok),
        FakeReq::new(Init::Fresh, Init::Done, Configure::Ok),
        None,
        None,
    )
    .await;
    assert!(matches!(state, State::Failed { .. }), "{state:?}");
    assert!(minted.is_none());
}

#[tokio::test]
async fn a_fresh_stack_mints_the_admin_and_wires_the_identity() {
    let (state, minted) = identity(
        media(Startup::Fresh, Create::Ok),
        FakeReq::new(Init::Fresh, Init::Done, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert_eq!(state, State::Wired);
    assert!(
        minted.is_some(),
        "the minted password is handed back to record"
    );
}

#[tokio::test]
async fn a_rejected_admin_creation_fails_with_the_services_own_words() {
    let (state, minted) = identity(
        media(Startup::Fresh, Create::Rejects),
        FakeReq::new(Init::Fresh, Init::Done, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Failed { .. }), "{state:?}");
    assert!(minted.is_none(), "a failed creation records no password");
}

#[tokio::test]
async fn a_jellyfin_set_up_outside_lemonfiber_is_skipped() {
    // The wizard has run but lemonfiber recorded no password, so the household set
    // it up: its credential is unknown, and the identity cannot be wired.
    let (state, minted) = identity(
        media(Startup::Completed, Create::Ok),
        FakeReq::new(Init::Fresh, Init::Done, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Skipped { .. }), "{state:?}");
    assert!(minted.is_none());
}

#[tokio::test]
async fn a_recorded_password_wires_an_already_initialised_seerr() {
    // Jellyfin was set up by lemonfiber before (password recorded) and Seerr is
    // already initialised: nothing is minted and nothing re-pointed.
    let (state, minted) = identity(
        media(Startup::Completed, Create::Ok),
        FakeReq::new(Init::Done, Init::Done, Configure::Ok),
        None,
        Some("minted-earlier"),
    )
    .await;
    assert_eq!(state, State::AlreadyWired);
    assert!(minted.is_none(), "an idempotent run mints nothing");
}

#[tokio::test]
async fn a_seerr_that_is_not_answering_still_records_the_minted_password() {
    // Jellyfin's account was created this run, so its password must be recorded
    // even though Seerr could not then be reached to finish the wiring.
    let (state, minted) = identity(
        media(Startup::Fresh, Create::Ok),
        FakeReq::new(Init::Down, Init::Done, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Skipped { .. }), "{state:?}");
    assert!(
        minted.is_some(),
        "the account holds the minted password, so it is recorded regardless"
    );
}

#[tokio::test]
async fn a_rejected_sign_in_fails() {
    let (state, _) = identity(
        media(Startup::Fresh, Create::Ok),
        FakeReq::new(Init::Fresh, Init::Done, Configure::Rejects),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Failed { .. }), "{state:?}");
}

#[tokio::test]
async fn a_sign_in_that_does_not_take_is_a_failure() {
    // Seerr accepted the sign-in but still reports itself uninitialised on the
    // read-back, so the write did not land.
    let (state, _) = identity(
        media(Startup::Fresh, Create::Ok),
        FakeReq::new(Init::Fresh, Init::Fresh, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Failed { .. }), "{state:?}");
}

#[tokio::test]
async fn a_read_back_that_cannot_be_reached_is_skipped() {
    let (state, _) = identity(
        media(Startup::Fresh, Create::Ok),
        FakeReq::new(Init::Fresh, Init::Down, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Skipped { .. }), "{state:?}");
}
