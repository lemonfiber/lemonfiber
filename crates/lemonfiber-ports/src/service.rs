//! Talking to the services themselves, to wire them to each other.
//!
//! One implementation per API *shape*, selected by the manifest's `api.kind` and
//! never by service name. Four applications share the Servarr shape, which is
//! what makes one client enough for them — and what lets a fork add a service
//! that reuses an existing shape with no Rust at all.

use std::time::Duration;

use async_trait::async_trait;

use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};

mod aggregators;
mod applications;
mod asking;
mod catalogue;
mod clients;
mod failure;
mod fetching;
mod household;
mod metering;
mod providers;
mod quality;
mod subtitles;
mod throttling;
mod trace;

pub use aggregators::{Aggregator, Aggregators, KnownAggregator};
pub use applications::{AppSync, Application, ApplicationKind, RegisteredApplication};
pub use asking::{Approving, Asking, Headroom, Left, Quota};
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
    Access, Allowed, Certificate, Household, Invited, Member, NamedLibrary, Unrated,
};
pub use metering::{Metering, Moved};
pub use providers::{
    IndexerUse, Indexers, Limits, Recorded, Standing, UsenetAccount, UsenetAccounts,
};
pub use quality::{MusicQuality, QualityReleases, ReleaseProbe};
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

/// One thing a household member asked for, as the request service records it.
///
/// The two statuses are carried as the service's own numbers rather than folded here:
/// what became of the request and what became of the media it asked for are separate
/// facts, and turning the pair into one word a member reads is a decision for the household
/// model above this, not for the code that reads them off the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HouseholdRequest {
    /// The number the request service files this request under, which is how one is
    /// named to it again when somebody rules on it.
    pub id: i64,
    /// When it was asked for, as the service timestamps it — what a request waiting
    /// on somebody is measured against, and what a counting period runs from.
    pub made: Option<String>,
    /// The member who asked, by the name the request service shows them under.
    pub member: String,
    /// Which service files the media — television or film — or `None` where the
    /// request service names a media type this build does not know.
    pub kind: Option<crate::media::Kind>,
    /// The id the \*arr filing this media knows it by, where the request service has
    /// handed it over yet. Nothing for a request still awaiting approval, which no
    /// \*arr has been told about — so the item cannot be named from the library, and
    /// is not claimed to be.
    pub item: Option<i64>,
    /// What became of the request, as the service numbers them.
    pub request_status: u8,
    /// What became of the media it asked for, as the service numbers them.
    pub media_status: u8,
}

/// What one member may ask for on the request service.
///
/// Only the half that bears on what a household chose. Everything else about the
/// account — what they are called, what they may watch — is the media server's to say,
/// and a second copy here would be a copy able to disagree with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requesting {
    /// The identifier this service tells them apart by.
    pub id: String,
    /// Whether what they ask for arrives without anybody approving it.
    ///
    /// True is the state a restriction has to undo: it is the whole of how a limit on
    /// watching and a lack of limit on requesting come apart.
    pub approves_own: bool,
}

/// A request manager's identity setup and the household's own requests — Seerr,
/// configured to authenticate its household against the media server rather than
/// against accounts of its own.
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

    /// Sign in through the media server as `username` with `password`, leaving the
    /// session the later reads are made under.
    ///
    /// Signing in is what [`Requests::configure_identity`] does first; this is that step
    /// on its own, for a read that must not also finish somebody's setup.
    ///
    /// **Where the media server is, is not named here**, and that is the difference
    /// between the two. A service that has been pointed at one already knows where it
    /// is, and naming it again is an attempt to point it somewhere — which it refuses,
    /// because moving a household's identity source out from under them is not a thing
    /// a sign-in should be able to do. So this opens a session on a service that is
    /// already set up, and [`Requests::configure_identity`] is the one that sets it up.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn sign_in(&self, username: &str, password: &str) -> Result<(), Failure>;

    /// Every request the household has made, across its members.
    ///
    /// Read as the owner, whose session sees the whole household: the members
    /// themselves have no way to run this, so the one account lemonfiber holds a
    /// credential for asks on their behalf.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn requests(&self) -> Result<Vec<HouseholdRequest>, Failure>;

    /// Give the request service an account for each of these media-server members.
    ///
    /// The link an invitation owes: the account exists on the media server from the
    /// moment somebody is invited, and this is what makes the same person known to
    /// the service they ask through.
    ///
    /// **Sending somebody it already knows is not an error and does nothing** — the
    /// service skips a member it already holds. So this is safe to call with everybody
    /// on every run, and a link that could not be made while the service was down is
    /// completed by the next run rather than by anything remembered in between.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn link_members(&self, members: &[String]) -> Result<(), Failure>;

    /// The account this service holds for a media-server member, where it holds one.
    ///
    /// `None` where it holds none — a member who has never signed in here is somebody
    /// this service has never heard of, which is **nothing to revoke** rather than a
    /// failure to revoke something.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn member_for(&self, media_server_id: &str) -> Result<Option<String>, Failure>;

    /// What one member may ask for here, by the media server's own identifier.
    ///
    /// `None` where this service holds no account for them, which is a member who has
    /// never signed in here rather than a read that failed.
    ///
    /// Wanted because a limit on what somebody may *watch* says nothing about what they
    /// may *ask for*, and the two disagreeing is the gap parental controls exist to
    /// close: a child who cannot watch something but can pull it into the library is a
    /// child whose parents' setting did half a job.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn requesting(&self, media_server_id: &str) -> Result<Option<Requesting>, Failure>;

    /// Make what this member asks for wait for somebody to approve it.
    ///
    /// **The narrowest thing this service can be told about a restricted member.** It
    /// has no notion of a content rating, so there is no limit here to mirror the media
    /// server's — what there is instead is the difference between a request that lands
    /// in the library unseen and one that an adult sees first. Taking the approval off
    /// leaves everything else about the account exactly as it was.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn approval_first(&self, id: &str) -> Result<(), Failure>;

    /// Take that account away, and with it everything it asked for.
    ///
    /// **This destroys their requests**, which is the service's own behaviour and not a
    /// choice made here: it removes them by hand so that a title still waiting goes back
    /// to being unrequested rather than being left pointing at nobody. Anything shown to
    /// an operator before this runs has to say so.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn remove_member(&self, id: &str) -> Result<(), Failure>;

    /// What the request service will tell the household about, as it stands.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn telling(&self) -> Result<Telling, Failure>;

    /// Set what it tells them about.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn tell(&self, telling: &Telling) -> Result<(), Failure>;

    /// The \*arrs it already hands requests to, by the endpoint each reaches.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn fulfilment_targets(&self) -> Result<Vec<RegisteredTarget>, Failure>;

    /// Hand it an \*arr to fulfil requests through.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn add_fulfilment_target(&self, target: &FulfilmentTarget) -> Result<(), Failure>;
}

/// Whether the request service reaches the household, and about what.
///
/// The occasions are a set, carried as the bit field the service keeps them in. It
/// is a number here rather than a list of named events because that is the shape the
/// service reads and writes, and translating it twice — once out, once back — would
/// be two places for the set to lose a member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Telling {
    /// Whether it will send anything at all.
    pub enabled: bool,
    /// Which occasions it sends on.
    pub occasions: u32,
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
