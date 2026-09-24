//! Talking to the services themselves, to wire them to each other.
//!
//! One implementation per API *shape*, selected by the manifest's `api.kind` and
//! never by service name. Four applications share the Servarr shape, which is
//! what makes one client enough for them — and what lets a fork add a service
//! that reuses an existing shape with no Rust at all.

use std::time::Duration;

use async_trait::async_trait;

use lemonfiber_error::{Code, Diagnose, Problem, Remedy, Severity, State};

mod addressing;
mod aggregators;
mod applications;
mod asking;
mod carrying;
mod catalogue;
mod clients;
mod failure;
mod fetching;
mod household;
mod metering;
mod notices;
mod providers;
mod quality;
mod requests;
pub mod stage;
mod subtitles;
mod throttling;
mod trace;

pub use addressing::{Address, Addressing};
pub use aggregators::{Aggregator, Aggregators, KnownAggregator};
pub use applications::{AppSync, Application, ApplicationKind, RegisteredApplication};
pub use asking::{Approving, Asking, Headroom, Holding, Left, Quota};
pub use carrying::{Carried, Carrying, Record};
pub use catalogue::{AddPlan, Added, Catalogue, CatalogueEntry};
pub use clients::{
    Category, ClientKind, ClientProbe, Credential, Download, DownloadClient, FulfilmentTarget,
    QualityProfile, Queue, QueueDepth, Queued, Queues, RegisteredClient, RegisteredFolder,
    RegisteredTarget, RootFolder, Seeded, Seeding, Transfers,
};
pub use failure::{
    Failure, ASK_FOR_REPAIRS, SERVICE_REFUSED, SERVICE_UNAUTHORISED, SERVICE_UNAVAILABLE,
    SERVICE_UNSUPPORTED,
};
pub use fetching::{Fetching, Pulling};
pub use household::{
    Access, Allowed, Certificate, Held, Household, Invited, Medium, Member, NamedLibrary, Unrated,
};
pub use metering::{Metering, Moved};
pub use notices::Noticing;
pub use providers::{
    IndexerUse, Indexers, Limits, Recorded, Standing, UsenetAccount, UsenetAccounts,
};
pub use quality::{MusicQuality, QualityReleases, ReleaseProbe};
pub use requests::{HouseholdRequest, Requesting, Requests, Telling};
pub use subtitles::{Subtitled, Subtitles, Watched, Watching};
pub use throttling::{Hours, Rates, Throttled, Throttling, Wanted, Window};
pub use trace::{FoundItem, ItemPart, Library, Pipeline, QueueItem, StuckItem, TraceEvent};

/// Who a service says it is, once it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// The service's own name for itself.
    pub name: String,
    /// The version it reports.
    pub version: String,
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

    /// Put one field of a download client back to a value, leaving everything else about
    /// it exactly as it is.
    ///
    /// Narrower than [`Self::update_download_client`] deliberately, and for one reason: a
    /// reversal knows the field it changed and what that field held, and nothing else. It
    /// does not know the client's credential — and it must not, because what a reversal
    /// reads from is a journal on disk, and a credential written there to make a reversal
    /// possible is a credential that did not need to exist.
    ///
    /// `None` where the field held nothing before, which removes it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable, does not hold the client, or
    /// refuses the change.
    async fn set_client_field(
        &self,
        id: &str,
        field: &str,
        value: Option<&str>,
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

    /// The quality profiles the service holds.
    ///
    /// Read rather than assumed, because the request service must name one when it
    /// hands over a request and the operator may have renamed or replaced the
    /// defaults. A service with none is not an error here — it is a service nothing
    /// can be fetched at, which the caller decides what to do about.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn quality_profiles(&self) -> Result<Vec<QualityProfile>, Failure>;
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

/// Telling a service how to move files from the download directory into the
/// library.
///
/// Its own port because it is neither provisioning nor a read: it is a correction
/// made once the filesystem has been observed, and only where the observation says
/// it is needed.
#[async_trait]
pub trait Importing: Send + Sync {
    /// Whether the service is currently set to hardlink rather than copy.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or answers unusably.
    async fn hardlinks(&self) -> Result<bool, Failure>;

    /// Set whether it should hardlink. `false` makes every import a copy, which
    /// is correct — and the only thing that works — where the volume cannot link.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn set_hardlinks(&self, hardlink: bool) -> Result<(), Failure>;
}

#[cfg(test)]
mod tests {
    use super::{
        Application, ApplicationKind, Category, ClientKind, Credential, Diagnose, DownloadClient,
        Failure, Identity, RegisteredApplication, RootFolder,
    };
    use lemonfiber_error::{Severity, State};

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
