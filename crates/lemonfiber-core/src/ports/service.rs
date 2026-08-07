//! Talking to the services themselves, to wire them to each other.
//!
//! One implementation per API *shape*, selected by the manifest's `api.kind` and
//! never by service name. Four applications share the Servarr shape, which is
//! what makes one client enough for them — and what lets a fork add a service
//! that reuses an existing shape with no Rust at all.

use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;

use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};

mod quality;
mod trace;

pub use quality::{MusicQuality, QualityReleases, ReleaseProbe};
pub use trace::{FoundItem, Library, Pipeline, QueueItem, StuckItem, TraceEvent};

/// Who a service says it is, once it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// The service's own name for itself.
    pub name: String,
    /// The version it reports.
    pub version: String,
}

/// Which download client an entry is, selecting the field schema the Servarr app
/// files it under — see the download-client contract in the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    /// `SABnzbd` — a Usenet client.
    Sabnzbd,
    /// `qBittorrent` — a torrent client.
    Qbittorrent,
}

/// How a download client proves itself, which differs by client — a single API
/// key for one, a username and its password for the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credential {
    /// A single API key.
    ApiKey(String),
    /// A username and its password.
    UserPass {
        /// The account name.
        username: String,
        /// Its password.
        password: String,
    },
}

/// The category a download is filed under, named after the media the requesting
/// application manages.
///
/// The field is not shared across applications — Sonarr names it `tvCategory`,
/// Radarr `movieCategory`, Lidarr `musicCategory` — so it travels with the client
/// rather than being assumed by the writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    /// The field the target application names its category.
    pub field: String,
    /// The value a download is filed under.
    pub value: String,
}

/// A download client, as one service needs to be told about another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadClient {
    /// The name the operator will see in the service's own interface.
    pub name: String,
    /// The host the service should reach it on.
    pub host: String,
    /// The port it listens on.
    pub port: u16,
    /// Which client it is, selecting the field schema.
    pub kind: ClientKind,
    /// How the service authenticates to it.
    pub credential: Credential,
    /// The category the requesting application files its downloads under.
    pub category: Category,
}

/// Where a service should file the media it imports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootFolder {
    /// The path inside the container.
    pub path: String,
    /// Which media type it holds.
    pub media_type: String,
}

/// A root folder a service already holds, with the identifier it gave it.
///
/// Read back so an absent connection can be told from one already made — matched
/// by path, not by any label — and so a later undo names exactly the one created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredFolder {
    /// The identifier the service assigned.
    pub id: String,
    /// The path it holds.
    pub path: String,
}

/// A download client a service already holds, with the identifier it gave it.
///
/// Read back so a client already registered can be told from an absent one —
/// matched by the endpoint it reaches, the host and port, rather than by its
/// label, so a differently-named but equivalent client is not duplicated — and so
/// a later undo names exactly the one created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredClient {
    /// The identifier the service assigned.
    pub id: String,
    /// The host the client is reached on.
    pub host: String,
    /// The port it listens on.
    pub port: u16,
    /// The category the client currently files under, where the service reports
    /// one — read back so an operator's change to it can be seen and preserved
    /// rather than reverted. Absent where the service names no category field.
    pub category: Option<Category>,
}

/// How a download client the service holds answered its reachability test —
/// the service's own verdict on whether the client it connects to is working,
/// keyed by the id the service assigned it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientProbe {
    /// The id of the client tested, matching a [`RegisteredClient::id`].
    pub id: String,
    /// Whether the client answered — `true` where the service reached it,
    /// `false` where the test failed.
    pub reachable: bool,
    /// What the service said when the test failed, where it said anything —
    /// carried so a warning can name why the client did not answer.
    pub detail: Option<String>,
}

/// Which media-filing \*arr a Prowlarr application entry syncs to, selecting the
/// field schema Prowlarr files it under and the release categories it syncs —
/// see the Prowlarr-application contract in the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationKind {
    /// Sonarr — television.
    Sonarr,
    /// Radarr — movies.
    Radarr,
    /// Lidarr — music.
    Lidarr,
}

/// A media-filing \*arr, as Prowlarr needs to be told about it so it syncs that
/// \*arr its indexers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Application {
    /// The name the operator will see in Prowlarr's own interface.
    pub name: String,
    /// Which application it is, selecting the field schema and sync categories.
    pub kind: ApplicationKind,
    /// The address the \*arr reaches Prowlarr back on, on the stack's network.
    pub prowlarr_url: String,
    /// The address Prowlarr reaches the \*arr on, on the stack's network — the
    /// connection an existing application is matched by.
    pub base_url: String,
    /// The \*arr's own API key, which is what lets Prowlarr write indexers into
    /// it.
    pub api_key: String,
}

/// An application Prowlarr already holds, with the identifier it gave it.
///
/// Read back so an application already registered can be told from an absent one
/// — matched by the address it reaches, the `base_url`, rather than by its label,
/// so a differently-named but equivalent application is not duplicated — and so a
/// later undo names exactly the one created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredApplication {
    /// The identifier Prowlarr assigned.
    pub id: String,
    /// The address it reaches the \*arr on.
    pub base_url: String,
}

/// How deep a service's queue is, and how much of it is stuck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueDepth {
    /// How many items are queued in total.
    pub total: usize,
    /// How many of them are stuck — warning or error — rather than progressing.
    pub stuck: usize,
}

/// A service refused, or could not be reached.
#[derive(Debug, Error)]
pub enum Failure {
    /// The service is not answering yet.
    #[error("`{service}` is not answering")]
    Unavailable {
        /// The service that was asked.
        service: String,
    },
    /// The service answered, and refused the credential.
    #[error("`{service}` rejected the credential")]
    Unauthorised {
        /// The service that refused.
        service: String,
    },
    /// The service answered with something unexpected.
    #[error("`{service}` answered unexpectedly: {detail}")]
    Refused {
        /// The service that answered.
        service: String,
        /// The service's own words.
        detail: String,
    },
    /// The service answered, but does not serve the API version lemonfiber speaks
    /// — it has been upgraded past it (or is older than it). Writing to it would
    /// mean writing something malformed, so it is reported instead.
    #[error("`{service}` does not serve the API version this build speaks: {detail}")]
    Unsupported {
        /// The service that answered.
        service: String,
        /// Which version is missing, in lemonfiber's words.
        detail: String,
    },
}

/// Raised when a service is not answering yet.
pub const SERVICE_UNAVAILABLE: Code = Code::new("SEED-1");

/// Raised when a service rejects the credential lemonfiber holds.
pub const SERVICE_UNAUTHORISED: Code = Code::new("SEED-2");

/// Raised when a service answers with something unusable.
pub const SERVICE_REFUSED: Code = Code::new("SEED-3");

/// Raised when a service does not serve the API version this build speaks.
pub const SERVICE_UNSUPPORTED: Code = Code::new("SEED-4");

impl Diagnose for Failure {
    fn problem(&self) -> Problem {
        match self {
            Self::Unavailable { service } => Problem::new(
                SERVICE_UNAVAILABLE,
                Severity::Warning,
                format!("{service} was not ready, so it was skipped"),
                "Wiring is resumable. Nothing was changed for this service, and running seed again picks it up once it is answering.",
                Remedy::new("Wait for it to finish starting, then run seed again")
                    .with_detail("lemonfiber seed"),
            ),
            Self::Unauthorised { service } => Problem::new(
                SERVICE_UNAUTHORISED,
                Severity::Error,
                format!("{service} rejected the credential"),
                "The credential lemonfiber holds no longer matches the one the service expects, usually because it was changed in the service's own interface.",
                Remedy::new("Re-read the service's credential").with_detail("lemonfiber doctor --fix"),
            )
            .in_state(State::Remediable),
            Self::Refused { service, detail } => Problem::unknown(
                SERVICE_REFUSED,
                Severity::Error,
                format!("{service} answered in a way lemonfiber did not expect"),
                "This is not a failure lemonfiber recognises, so it will not guess at what would fix it.",
            )
            .with_detail(detail.clone()),
            Self::Unsupported { service, detail } => Problem::new(
                SERVICE_UNSUPPORTED,
                Severity::Error,
                format!("{service} does not serve the API version this build speaks"),
                "The service was upgraded past — or stands before — the API version lemonfiber knows how to speak, so writing to it would mean writing something malformed. Nothing was changed for it.",
                Remedy::new("Match the service to the version lemonfiber supports, or update lemonfiber, then run seed again")
                    .with_detail("lemonfiber seed"),
            )
            .with_detail(detail.clone()),
        }
    }
}

/// One API shape lemonfiber knows how to speak.
///
/// Every write is journalled and checked against the operator's own changes
/// first, so seeding a stack that has been tuned by hand preserves the tuning
/// rather than reverting it.
#[async_trait]
pub trait Client: Send + Sync {
    /// Ask the service who it is, confirming it is up and the credential works.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unavailable`] when it is not answering, and
    /// [`Failure::Unauthorised`] when the credential is refused.
    async fn identity(&self) -> Result<Identity, Failure>;

    /// Tell the service about a download client.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn register_download_client(&self, client: &DownloadClient) -> Result<(), Failure>;

    /// Rewrite a download client the service already holds to lemonfiber's settings,
    /// named by the id the service assigned it — the update a reset makes to revert a
    /// drifted category to the one lemonfiber files under, in place rather than as a
    /// second client.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn update_download_client(
        &self,
        id: &str,
        client: &DownloadClient,
    ) -> Result<(), Failure>;

    /// Ask the service to test every download client it holds, reporting whether
    /// each answered — the service's own verdict, one entry per client keyed by id.
    ///
    /// The service is the authority on whether its download client is reachable: it
    /// is the one that connects to the client, not lemonfiber, which sits on the host
    /// and cannot reach the client's in-network address. Tested all at once because
    /// the service tests them together, and read only where a drift is found, to
    /// escalate a drifted client to a warning solely when the drift left it
    /// unreachable.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service itself is unreachable or refuses the
    /// request — distinct from a client that answered the test as unreachable, which
    /// is a per-client [`ClientProbe`] with `reachable` false, not an error.
    async fn test_download_clients(&self) -> Result<Vec<ClientProbe>, Failure>;

    /// Tell the service where to file what it imports.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn register_root_folder(&self, folder: &RootFolder) -> Result<(), Failure>;

    /// The root folders the service already has.
    ///
    /// Read so a connection already made is left alone rather than duplicated,
    /// and so a write can be confirmed by reading it back.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn root_folders(&self) -> Result<Vec<RegisteredFolder>, Failure>;

    /// The download clients the service already has, each by the endpoint it
    /// reaches rather than its label.
    ///
    /// Read so a client already registered is left alone rather than duplicated,
    /// and so a registration can be confirmed by reading it back.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn download_clients(&self) -> Result<Vec<RegisteredClient>, Failure>;
}

/// Reading a service's queue for the dashboard.
///
/// A port of its own, not a method on [`Client`], because it is a read-only
/// telemetry capability the dashboard uses rather than part of the wiring shape
/// seeding drives — keeping it apart means the fakes that stand in for a service
/// being wired need not answer for a question they are never asked.
#[async_trait]
pub trait Queues: Send + Sync {
    /// How deep the service's queue is, and how much of it is stuck — the numbers
    /// the dashboard shows without opening the service's own web UI.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn queue(&self) -> Result<QueueDepth, Failure>;
}

/// One active download a client is working, normalised to what the dashboard
/// shows — the point at which each client's own idea of progress becomes the
/// same three figures.
///
/// The protocol is not carried here: it is intrinsic to which client answered —
/// a torrent from qBittorrent, a Usenet download from `SABnzbd` — so the gatherer
/// sets it from the target it asked rather than trusting each client to name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    /// What is being downloaded.
    pub name: String,
    /// How far along, from zero to a hundred.
    pub progress: u8,
    /// The current speed in bytes per second, or `None` where the client reported
    /// none — kept apart from a reported zero, which is a download that is stalled
    /// rather than one whose speed is unknown.
    pub speed: Option<u64>,
    /// The time left, or `None` where the client gives none because it is stalled
    /// or cannot estimate one.
    pub eta: Option<Duration>,
    /// The bytes still to be written to disk for this download, or `None` where the
    /// client reports no figure — kept apart from a reported zero, which is a
    /// download already complete rather than one whose size is unknown. Summed
    /// across a stack's clients, this is the committed content the free-space
    /// projection weighs against what the volume has left.
    pub remaining: Option<u64>,
}

/// Reading a download client's active transfers for the dashboard.
///
/// Like [`Queues`], a read-only telemetry port kept off the wiring [`Client`]: the
/// dashboard asks a download client what it is moving right now, a question
/// seeding never needs, so the fakes that stand in for wiring need not answer it.
#[async_trait]
pub trait Transfers: Send + Sync {
    /// The downloads the client is working right now — each one's name, progress,
    /// speed and time left, the figures the dashboard shows without opening the
    /// client's own web UI.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the client is unreachable or refuses.
    async fn transfers(&self) -> Result<Vec<Download>, Failure>;
}

/// Prowlarr's application sync — the one Servarr-shape service that manages other
/// Servarr applications rather than media.
///
/// It is a port of its own, not a method on [`Client`], because only Prowlarr has
/// applications and it versions its API a major behind the media \*arrs; a method
/// on the shared shape would be a capability the others do not have.
#[async_trait]
pub trait AppSync: Send + Sync {
    /// Tell Prowlarr about a media-filing \*arr, so it syncs that \*arr its
    /// indexers.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when Prowlarr is unreachable or refuses.
    async fn register_application(&self, application: &Application) -> Result<(), Failure>;

    /// The applications Prowlarr already holds, each by the address it reaches
    /// the \*arr on rather than its label.
    ///
    /// Read so an application already registered is left alone rather than
    /// duplicated, and so a registration can be confirmed by reading it back.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when Prowlarr is unreachable or refuses.
    async fn applications(&self) -> Result<Vec<RegisteredApplication>, Failure>;
}

/// A media server's first-run setup — Jellyfin, the one service lemonfiber
/// creates an account on rather than reading a key from.
///
/// Jellyfin writes no key to disk and asks for its first account through a setup
/// wizard, so the credential is one lemonfiber mints and sets here rather than
/// reads elsewhere.
#[async_trait]
pub trait MediaServer: Send + Sync {
    /// Whether the first-run setup is already done — the gate that keeps a
    /// completed wizard, and the household's own account, untouched.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn startup_completed(&self) -> Result<bool, Failure>;

    /// Create the administrator account and finish setup, in one step because the
    /// setup endpoints answer only until it is complete.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn create_admin(&self, name: &str, password: &str) -> Result<(), Failure>;
}

/// A request manager's identity setup — Seerr, configured to authenticate its
/// household against the media server rather than against accounts of its own.
#[async_trait]
pub trait Requests: Send + Sync {
    /// Whether it has already been initialised — the gate that never re-points a
    /// running instance's identity source and so keeps its existing sign-ins.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn initialized(&self) -> Result<bool, Failure>;

    /// Point authentication at the media server reached at `server_url`, signing
    /// in as `username` with `password` — which on the first call also creates the
    /// owner from that account.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn configure_identity(
        &self,
        username: &str,
        password: &str,
        server_url: &str,
    ) -> Result<(), Failure>;
}

/// Asking a Servarr-shape service to run one of its background commands — the
/// operator-triggered maintenance a stack sometimes needs, such as re-searching
/// existing content for a better release when the quality bar is raised.
#[async_trait]
pub trait Maintenance: Send + Sync {
    /// Ask the service to run the named command. Returns once the service has
    /// accepted it; the work itself then runs in the background there, so this is
    /// the request to start it, not a wait for it to finish.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses the command.
    async fn run_command(&self, name: &str) -> Result<(), Failure>;
}

#[cfg(test)]
mod tests {
    use super::{
        Application, ApplicationKind, Category, ClientKind, Credential, Diagnose, DownloadClient,
        Failure, Identity, RegisteredApplication, RootFolder,
    };
    use crate::error::{Severity, State};

    #[test]
    fn an_absent_service_is_skipped_rather_than_failed() {
        let problem = Failure::Unavailable {
            service: "sonarr".to_owned(),
        }
        .problem();
        assert_eq!(problem.severity, Severity::Warning);
        assert!(problem.summary.contains("skipped"));
    }

    #[test]
    fn a_rejected_credential_is_something_lemonfiber_can_fix() {
        let problem = Failure::Unauthorised {
            service: "sonarr".to_owned(),
        }
        .problem();
        assert_eq!(problem.state, State::Remediable);
    }

    #[test]
    fn an_unrecognised_answer_admits_ignorance_rather_than_guessing() {
        let problem = Failure::Refused {
            service: "sonarr".to_owned(),
            detail: "500 Internal Server Error".to_owned(),
        }
        .problem();
        assert_eq!(problem.state, State::Unknown);
        assert_eq!(problem.detail.as_deref(), Some("500 Internal Server Error"));
        assert!(!problem.remedies.is_empty(), "escalation is still offered");
    }

    #[test]
    fn an_unsupported_api_version_is_reported_with_a_remedy() {
        let problem = Failure::Unsupported {
            service: "sonarr".to_owned(),
            detail: "there is no /api/v3".to_owned(),
        }
        .problem();
        assert_eq!(problem.severity, Severity::Error);
        assert_eq!(problem.detail.as_deref(), Some("there is no /api/v3"));
        assert!(
            !problem.remedies.is_empty(),
            "aligning the versions is offered as the way out"
        );
    }

    #[test]
    fn every_failure_names_the_service_it_is_about() {
        let failures = [
            Failure::Unavailable {
                service: "sonarr".to_owned(),
            },
            Failure::Unauthorised {
                service: "sonarr".to_owned(),
            },
            Failure::Refused {
                service: "sonarr".to_owned(),
                detail: "boom".to_owned(),
            },
            Failure::Unsupported {
                service: "sonarr".to_owned(),
                detail: "boom".to_owned(),
            },
        ];
        for failure in &failures {
            assert!(failure.to_string().contains("sonarr"));
            assert!(!failure.problem().remedies.is_empty());
        }
    }

    #[test]
    fn the_things_a_service_is_told_about_are_plain_data() {
        let identity = Identity {
            name: "Sonarr".to_owned(),
            version: "4.0.15".to_owned(),
        };
        assert_eq!(identity.clone(), identity);

        let client = DownloadClient {
            name: "SABnzbd".to_owned(),
            host: "sabnzbd".to_owned(),
            port: 8080,
            kind: ClientKind::Sabnzbd,
            credential: Credential::ApiKey("the-key".to_owned()),
            category: Category {
                field: "tvCategory".to_owned(),
                value: "tv".to_owned(),
            },
        };
        assert_eq!(client.clone().port, 8080);

        let folder = RootFolder {
            path: "/data/media/tv".to_owned(),
            media_type: "tv".to_owned(),
        };
        assert_eq!(folder.clone().media_type, "tv");

        let application = Application {
            name: "Sonarr".to_owned(),
            kind: ApplicationKind::Sonarr,
            prowlarr_url: "http://prowlarr:9696".to_owned(),
            base_url: "http://sonarr:8989".to_owned(),
            api_key: "the-key".to_owned(),
        };
        assert_eq!(application.clone().kind, ApplicationKind::Sonarr);

        let registered = RegisteredApplication {
            id: "3".to_owned(),
            base_url: "http://sonarr:8989".to_owned(),
        };
        assert_eq!(registered.clone(), registered);
    }
}
