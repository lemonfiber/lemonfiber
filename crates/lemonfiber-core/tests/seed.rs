//! The seed policy and the driver that carries it out.
//!
//! The policy is pure and could be tested in place; the driver speaks the service
//! port, an async trait, so both are driven from here — where a fake service
//! stands in and the driver's coverage is counted from the one compiled copy.

use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::random::Random;
use lemonfiber_core::ports::service::{
    AppSync, Application, ApplicationKind, Category, Client, ClientKind, Credential,
    DownloadClient, Failure, Identity, MediaServer, RegisteredApplication, RegisteredClient,
    RegisteredFolder, Requests, RootFolder,
};
use lemonfiber_core::seed::{
    intent, wire_applications, wire_download_clients, wire_jellyfin_identity, wire_root_folders,
    Intent, Observed, Report, State, Wiring,
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
}

/// A service that answers the seed driver from a script.
struct FakeService {
    mode: Mode,
    folders: Mutex<Vec<RegisteredFolder>>,
    clients: Mutex<Vec<RegisteredClient>>,
    reads: Mutex<u32>,
    next_id: Mutex<u32>,
}

impl FakeService {
    fn with(mode: Mode, folders: Vec<RegisteredFolder>) -> Self {
        Self {
            mode,
            folders: Mutex::new(folders),
            clients: Mutex::new(Vec::new()),
            reads: Mutex::new(0),
            next_id: Mutex::new(100),
        }
    }

    fn with_clients(mode: Mode, clients: Vec<RegisteredClient>) -> Self {
        Self {
            mode,
            folders: Mutex::new(Vec::new()),
            clients: Mutex::new(clients),
            reads: Mutex::new(0),
            next_id: Mutex::new(100),
        }
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
/// number of changes journalled.
async fn seed(service: FakeService, wanted: &[RootFolder]) -> (Vec<State>, usize) {
    let mut journal = Journal::new();
    let wirings = wire_root_folders(&service, "sonarr", wanted, &mut journal, "t").await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, journal.changes().len())
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
    DownloadClient {
        name: name.to_owned(),
        host: host.to_owned(),
        port,
        kind: ClientKind::Sabnzbd,
        credential: Credential::ApiKey("sab-key".to_owned()),
        category: Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        },
    }
}

/// Run the client driver for the wanted clients, returning their resulting states
/// and the number of changes journalled.
async fn seed_clients(service: FakeService, wanted: &[DownloadClient]) -> (Vec<State>, usize) {
    let mut journal = Journal::new();
    let wirings = wire_download_clients(&service, "sonarr", wanted, &mut journal, "t").await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, journal.changes().len())
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
    // Same endpoint, but the operator changed the category in the *arr itself.
    // That is their edit to keep, not a mistake to revert: it is reported as
    // drift and left exactly as it is, nothing re-registered.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "qbittorrent".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "my-own-tv".to_owned(),
        }),
    }];
    let (states, recorded) = seed_clients(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("qBittorrent", "qbittorrent", 8080)],
    )
    .await;
    assert_eq!(states, vec![State::Drifted]);
    assert_eq!(
        recorded, 0,
        "an operator's own change is preserved, not rewritten"
    );
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
