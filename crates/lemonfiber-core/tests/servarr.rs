//! The Servarr client, driven through the HTTP port against a fake transport.
//!
//! The client turns a request into an API call and reads what the service
//! answered; the fake is that service, replying with exactly the status and body
//! a test wants — so every branch of the identity and registration paths is
//! exercised with nothing running. The client speaks an async trait built on
//! another, so it is driven from here rather than from an in-crate test, where it
//! would be compiled twice and its coverage counted from the wrong copy.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_core::ports::http::{Http, Request, Response, Unreachable};
use lemonfiber_core::ports::service::{
    Client, DownloadClient, Failure, RegisteredFolder, RootFolder,
};
use lemonfiber_core::servarr::{api_key, Servarr};

/// What the fake transport answers with.
enum Answer {
    /// A response with this status and body.
    Reply(u16, &'static str),
    /// Nothing answered.
    Silent,
}

/// A transport that answers every request the same way, keeping the last one.
struct Fake {
    answer: Answer,
    seen: Mutex<Option<Request>>,
}

impl Fake {
    fn new(answer: Answer) -> Arc<Self> {
        Arc::new(Self {
            answer,
            seen: Mutex::new(None),
        })
    }

    /// The request the client sent.
    fn request(&self) -> Option<Request> {
        self.seen.lock().ok().and_then(|guard| guard.clone())
    }
}

#[async_trait]
impl Http for Fake {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        if let Ok(mut guard) = self.seen.lock() {
            *guard = Some(request.clone());
        }
        match self.answer {
            Answer::Reply(status, body) => Ok(Response {
                status,
                body: body.to_owned(),
            }),
            Answer::Silent => Err(Unreachable {
                url: request.url.clone(),
                reason: "connection refused".to_owned(),
            }),
        }
    }
}

/// A Sonarr client over the given fake.
fn sonarr(fake: &Arc<Fake>) -> Servarr {
    let http: Arc<dyn Http> = fake.clone();
    Servarr::new(http, "http://sonarr:8989", "the-key", "sonarr")
}

#[tokio::test]
async fn a_valid_credential_reads_the_service_identity() {
    let fake = Fake::new(Answer::Reply(
        200,
        r#"{"instanceName":"Sonarr","appName":"Sonarr","version":"4.0.15.2941"}"#,
    ));
    let identity = sonarr(&fake).identity().await;
    assert_eq!(
        identity.ok().map(|who| (who.name, who.version)),
        Some(("Sonarr".to_owned(), "4.0.15.2941".to_owned()))
    );

    // The credential rode the right header to the right route.
    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|request| request.url.ends_with("/api/v3/system/status")));
    assert!(sent.is_some_and(|request| request
        .headers
        .iter()
        .any(|(name, value)| name == "X-Api-Key" && value == "the-key")));
}

#[tokio::test]
async fn the_app_name_is_used_when_no_instance_name_is_set() {
    let fake = Fake::new(Answer::Reply(
        200,
        r#"{"appName":"Radarr","version":"5.0"}"#,
    ));
    let identity = sonarr(&fake).identity().await;
    assert_eq!(identity.ok().map(|who| who.name), Some("Radarr".to_owned()));
}

#[tokio::test]
async fn a_rejected_key_is_unauthorised() {
    let fake = Fake::new(Answer::Reply(401, ""));
    assert!(matches!(
        sonarr(&fake).identity().await,
        Err(Failure::Unauthorised { .. })
    ));
}

#[tokio::test]
async fn a_service_that_is_not_answering_is_unavailable() {
    let fake = Fake::new(Answer::Silent);
    assert!(matches!(
        sonarr(&fake).identity().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn an_unexpected_status_is_refused_with_the_services_own_words() {
    // The service's own message is carried through, not paraphrased.
    let fake = Fake::new(Answer::Reply(500, "database is locked"));
    let detail = match sonarr(&fake).identity().await {
        Err(Failure::Refused { detail, .. }) => Some(detail),
        _ => None,
    };
    assert!(
        detail.is_some_and(|words| words.contains("500") && words.contains("database is locked")),
        "the service's own words are carried through"
    );
}

#[tokio::test]
async fn an_unexpected_status_with_no_body_is_refused_with_its_code() {
    let fake = Fake::new(Answer::Reply(503, ""));
    let detail = match sonarr(&fake).identity().await {
        Err(Failure::Refused { detail, .. }) => Some(detail),
        _ => None,
    };
    assert_eq!(detail.as_deref(), Some("HTTP 503"));
}

#[tokio::test]
async fn an_unreadable_status_body_is_refused() {
    let fake = Fake::new(Answer::Reply(200, "not json at all"));
    assert!(matches!(
        sonarr(&fake).identity().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_status_that_names_neither_itself_nor_its_version_is_refused() {
    let fake = Fake::new(Answer::Reply(200, r#"{"version":"4.0"}"#));
    assert!(matches!(
        sonarr(&fake).identity().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_root_folder_is_posted_to_its_endpoint() {
    let fake = Fake::new(Answer::Reply(201, ""));
    let folder = RootFolder {
        path: "/data/media/tv".to_owned(),
        media_type: "tv".to_owned(),
    };
    assert!(sonarr(&fake).register_root_folder(&folder).await.is_ok());

    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|request| request.url.ends_with("/api/v3/rootfolder")));
    assert!(sent.is_some_and(|request| request
        .body
        .is_some_and(|body| body.contains("/data/media/tv"))));
}

#[tokio::test]
async fn a_download_client_is_posted_to_its_endpoint() {
    let fake = Fake::new(Answer::Reply(201, ""));
    let client = DownloadClient {
        name: "SABnzbd".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        credential: None,
    };
    assert!(sonarr(&fake)
        .register_download_client(&client)
        .await
        .is_ok());

    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|request| request.url.ends_with("/api/v3/downloadclient")));
    assert!(sent.is_some_and(|request| request.body.is_some_and(|body| body.contains("SABnzbd"))));
}

#[tokio::test]
async fn a_rejected_root_folder_registration_is_refused() {
    let fake = Fake::new(Answer::Reply(400, "path already used"));
    let folder = RootFolder {
        path: "/data/media/tv".to_owned(),
        media_type: "tv".to_owned(),
    };
    assert!(matches!(
        sonarr(&fake).register_root_folder(&folder).await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_rejected_download_client_registration_is_refused() {
    let fake = Fake::new(Answer::Reply(400, "unknown implementation"));
    let client = DownloadClient {
        name: "SABnzbd".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        credential: None,
    };
    assert!(matches!(
        sonarr(&fake).register_download_client(&client).await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn the_root_folders_are_read_back_with_their_ids() {
    let fake = Fake::new(Answer::Reply(
        200,
        r#"[{"id":1,"path":"/data/media/tv"},{"id":7,"path":"/data/media/movies"}]"#,
    ));
    let folders = sonarr(&fake).root_folders().await;
    assert_eq!(
        folders.ok(),
        Some(vec![
            RegisteredFolder {
                id: "1".to_owned(),
                path: "/data/media/tv".to_owned(),
            },
            RegisteredFolder {
                id: "7".to_owned(),
                path: "/data/media/movies".to_owned(),
            },
        ])
    );
}

#[tokio::test]
async fn an_unreadable_folder_list_is_refused() {
    let fake = Fake::new(Answer::Reply(200, "not an array"));
    assert!(matches!(
        sonarr(&fake).root_folders().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_rejected_folder_listing_is_unauthorised() {
    let fake = Fake::new(Answer::Reply(401, ""));
    assert!(matches!(
        sonarr(&fake).root_folders().await,
        Err(Failure::Unauthorised { .. })
    ));
}

#[tokio::test]
async fn a_folder_listing_with_no_answer_is_unavailable() {
    let fake = Fake::new(Answer::Silent);
    assert!(matches!(
        sonarr(&fake).root_folders().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[test]
fn a_generated_api_key_is_read_from_the_config() {
    let config = "<Config>\n  <Port>8989</Port>\n  <ApiKey>a1b2c3d4e5</ApiKey>\n</Config>";
    assert_eq!(api_key(config).as_deref(), Some("a1b2c3d4e5"));
}

#[test]
fn surrounding_whitespace_in_the_key_element_is_trimmed() {
    let config = "<ApiKey>\n    a1b2c3d4e5\n  </ApiKey>";
    assert_eq!(api_key(config).as_deref(), Some("a1b2c3d4e5"));
}

#[test]
fn a_key_not_generated_yet_is_absent_not_a_fault() {
    // The element is present but empty until first start completes.
    assert_eq!(api_key("<Config><ApiKey></ApiKey></Config>"), None);
    // Present but only whitespace trims to empty, which is also not-yet.
    assert_eq!(api_key("<Config><ApiKey>   </ApiKey></Config>"), None);
    // Or the element is not there at all yet.
    assert_eq!(api_key("<Config><Port>8989</Port></Config>"), None);
}

#[test]
fn a_multibyte_key_survives_intact() {
    // The offsets are byte offsets at ASCII tag boundaries, so a key with
    // multibyte characters is read whole rather than split mid-codepoint.
    let config = "<Config><ApiKey>café☃clé</ApiKey></Config>";
    assert_eq!(api_key(config).as_deref(), Some("café☃clé"));
}

#[test]
fn a_truncated_config_yields_no_key_rather_than_a_panic() {
    // The opening tag is there but the file was read mid-write, so there is no
    // close: no key, and no crash on the missing end.
    assert_eq!(api_key("<Config><ApiKey>a1b2c3"), None);
}
