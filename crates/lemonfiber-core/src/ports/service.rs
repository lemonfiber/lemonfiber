//! Talking to the services themselves, to wire them to each other.
//!
//! One implementation per API *shape*, selected by the manifest's `api.kind` and
//! never by service name. Four applications share the Servarr shape, which is
//! what makes one client enough for them — and what lets a fork add a service
//! that reuses an existing shape with no Rust at all.

use async_trait::async_trait;
use thiserror::Error;

use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};

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
}

/// Raised when a service is not answering yet.
pub const SERVICE_UNAVAILABLE: Code = Code::new("SEED-1");

/// Raised when a service rejects the credential lemonfiber holds.
pub const SERVICE_UNAUTHORISED: Code = Code::new("SEED-2");

/// Raised when a service answers with something unusable.
pub const SERVICE_REFUSED: Code = Code::new("SEED-3");

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

#[cfg(test)]
mod tests {
    use super::{
        Category, ClientKind, Credential, Diagnose, DownloadClient, Failure, Identity, RootFolder,
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
    }
}
