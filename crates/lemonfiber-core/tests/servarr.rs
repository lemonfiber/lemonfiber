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
use lemonfiber_core::audio::Format;
use lemonfiber_core::ports::http::{Http, Method, Request, Response, Unreachable};
use lemonfiber_core::ports::service::{
    Category, Client, ClientKind, Credential, DownloadClient, Failure, Importing, Maintenance,
    MusicQuality, Pipeline, QueueDepth, QueueItem, Queued, Queues, RegisteredClient,
    RegisteredFolder, RootFolder,
};
use lemonfiber_core::recyclarr::Kind;
use lemonfiber_core::servarr::{api_key, Servarr};
use lemonfiber_core::trace::{Outcome, Stage};

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
                attempts: 1,
            }),
        }
    }
}

/// A Sonarr client over the given fake — the v3 the media *arrs answer at.
fn sonarr(fake: &Arc<Fake>) -> Servarr {
    let http: Arc<dyn Http> = fake.clone();
    Servarr::new(http, "http://sonarr:8989", "the-key", "sonarr", 3)
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
async fn a_command_is_posted_by_name_and_accepted() {
    let fake = Fake::new(Answer::Reply(201, r#"{"name":"CutoffUnmetEpisodeSearch"}"#));
    let accepted = sonarr(&fake).run_command("CutoffUnmetEpisodeSearch").await;
    assert!(accepted.is_ok());

    // The command rode a POST to the command route, named in the body.
    let sent = fake.request();
    assert!(sent.as_ref().is_some_and(
        |request| request.method == Method::Post && request.url.ends_with("/api/v3/command")
    ));
    assert!(sent.is_some_and(|request| request
        .body
        .as_deref()
        .is_some_and(|body| body.contains("CutoffUnmetEpisodeSearch"))));
}

#[tokio::test]
async fn a_command_a_service_refuses_is_a_failure() {
    let fake = Fake::new(Answer::Reply(500, "boom"));
    assert!(sonarr(&fake)
        .run_command("CutoffUnmetEpisodeSearch")
        .await
        .is_err());
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
async fn an_unreadable_status_body_is_refused_and_the_detail_names_the_break() {
    let fake = Fake::new(Answer::Reply(200, "not json at all"));
    let detail = match sonarr(&fake).identity().await {
        Err(Failure::Refused { detail, .. }) => detail,
        _ => String::new(),
    };
    // The generic phrase, then the parser's own account of what failed — kept
    // rather than paraphrased, so a shape change is diagnosable.
    assert!(
        detail.starts_with("the status response could not be read: "),
        "missing the generic lead-in: {detail}"
    );
    assert!(
        detail.contains("expected") || detail.contains("column"),
        "the parser's own words should survive: {detail}"
    );
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
async fn a_service_that_does_not_serve_the_api_version_is_unsupported() {
    // A 404 is the whole /api/v{n} prefix not served — the service was upgraded
    // past (or stands before) the version this build speaks. Reported as
    // unsupported, naming the version, rather than read as a generic refusal so
    // seeding refuses it rather than writing something malformed.
    let fake = Fake::new(Answer::Reply(404, ""));
    let detail = match sonarr(&fake).identity().await {
        Err(Failure::Unsupported { detail, .. }) => Some(detail),
        _ => None,
    };
    assert!(
        detail.is_some_and(|words| words.contains("/api/v3")),
        "the unsupported version is named"
    );
}

#[tokio::test]
async fn a_read_against_an_unsupported_api_version_is_unsupported_too() {
    // The seed read path shares the probe, so a 404 on the folder list is the same
    // unsupported-version signal — and it is the read, before any write, so nothing
    // malformed is ever posted.
    let fake = Fake::new(Answer::Reply(404, ""));
    assert!(matches!(
        sonarr(&fake).root_folders().await,
        Err(Failure::Unsupported { .. })
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

/// A `SABnzbd` download client: a Usenet client authenticated by an API key.
fn sabnzbd() -> DownloadClient {
    DownloadClient {
        name: "SABnzbd".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        kind: ClientKind::Sabnzbd,
        credential: Credential::ApiKey("sab-key".to_owned()),
        category: Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        },
    }
}

/// A `qBittorrent` download client: a torrent client authenticated by a login.
fn qbit() -> DownloadClient {
    DownloadClient {
        name: "qBittorrent".to_owned(),
        host: "qbittorrent".to_owned(),
        port: 8081,
        kind: ClientKind::Qbittorrent,
        credential: Credential::UserPass {
            username: "admin".to_owned(),
            password: "web-pass".to_owned(),
        },
        category: Category {
            field: "movieCategory".to_owned(),
            value: "movies".to_owned(),
        },
    }
}

#[tokio::test]
async fn a_sabnzbd_client_is_posted_as_its_usenet_implementation() {
    let fake = Fake::new(Answer::Reply(201, ""));
    assert!(sonarr(&fake)
        .register_download_client(&sabnzbd())
        .await
        .is_ok());

    let body = fake
        .request()
        .and_then(|request| request.body)
        .unwrap_or_default();
    for expected in [
        r#""implementation":"Sabnzbd""#,
        r#""configContract":"SabnzbdSettings""#,
        r#""protocol":"usenet""#,
        r#""name":"apiKey""#,
        "sab-key",
        r#""name":"tvCategory""#,
    ] {
        assert!(
            body.contains(expected),
            "SABnzbd body missing {expected}: {body}"
        );
    }
    // A Usenet client is not told a username and password.
    assert!(!body.contains(r#""name":"username""#));
}

#[tokio::test]
async fn a_qbittorrent_client_is_posted_as_its_torrent_implementation() {
    let fake = Fake::new(Answer::Reply(201, ""));
    assert!(sonarr(&fake)
        .register_download_client(&qbit())
        .await
        .is_ok());

    let body = fake
        .request()
        .and_then(|request| request.body)
        .unwrap_or_default();
    for expected in [
        r#""implementation":"QBittorrent""#,
        r#""configContract":"QBittorrentSettings""#,
        r#""protocol":"torrent""#,
        r#""name":"username""#,
        r#""name":"password""#,
        "web-pass",
        r#""name":"movieCategory""#,
    ] {
        assert!(
            body.contains(expected),
            "qBittorrent body missing {expected}: {body}"
        );
    }
    // A torrent client authenticated by login is not told an API key.
    assert!(!body.contains(r#""name":"apiKey""#));
}

#[tokio::test]
async fn a_download_client_is_posted_to_its_endpoint() {
    let fake = Fake::new(Answer::Reply(201, ""));
    assert!(sonarr(&fake)
        .register_download_client(&sabnzbd())
        .await
        .is_ok());

    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|request| request.url.ends_with("/api/v3/downloadclient")));
    assert!(sent.is_some_and(|request| request.body.is_some_and(|body| body.contains("SABnzbd"))));
}

#[tokio::test]
async fn an_updated_download_client_is_put_to_its_id_carrying_it() {
    let fake = Fake::new(Answer::Reply(200, ""));
    assert!(sonarr(&fake)
        .update_download_client("7", &sabnzbd())
        .await
        .is_ok());

    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|request| request.method == Method::Put
            && request.url.ends_with("/api/v3/downloadclient/7")));
    // The document rewrites the one that is there rather than adding a second: it names
    // the id the service assigned.
    assert!(sent.is_some_and(|request| request.body.is_some_and(|body| body.contains(r#""id":7"#))));
}

#[tokio::test]
async fn an_update_with_an_id_the_service_did_not_assign_is_refused() {
    // A non-numeric id is not one a Servarr service assigns, so there is nothing to
    // address — refused rather than a malformed request sent.
    let fake = Fake::new(Answer::Reply(200, ""));
    assert!(matches!(
        sonarr(&fake)
            .update_download_client("not-a-number", &sabnzbd())
            .await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_rejected_download_client_update_is_refused() {
    let fake = Fake::new(Answer::Reply(400, "cannot update"));
    assert!(matches!(
        sonarr(&fake).update_download_client("7", &sabnzbd()).await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn testing_download_clients_reads_each_services_verdict() {
    // `testall` answers one result per client: the valid one is reachable with nothing
    // to say, the one that failed carries the service's joined words, and one that
    // failed without words carries none — never an error, since a failing client is
    // the answer wanted, not a fault.
    let body = r#"[
        {"id":1,"isValid":true,"validationFailures":[]},
        {"id":2,"isValid":false,"validationFailures":[
            {"errorMessage":"unable to connect"},{"errorMessage":"timed out"}]},
        {"id":3,"isValid":false,"validationFailures":[]}
    ]"#;
    let fake = Fake::new(Answer::Reply(200, body));
    let probes = sonarr(&fake)
        .test_download_clients()
        .await
        .unwrap_or_default();

    // The test is asked of the service's own `testall`.
    assert!(fake
        .request()
        .is_some_and(|request| request.method == Method::Post
            && request.url.ends_with("/api/v3/downloadclient/testall")));

    let reachable = probes.iter().find(|probe| probe.id == "1");
    assert!(reachable.is_some_and(|probe| probe.reachable && probe.detail.is_none()));
    let refused = probes.iter().find(|probe| probe.id == "2");
    assert!(refused.is_some_and(|probe| !probe.reachable
        && probe
            .detail
            .as_deref()
            .is_some_and(|detail| detail == "unable to connect; timed out")));
    let wordless = probes.iter().find(|probe| probe.id == "3");
    assert!(wordless.is_some_and(|probe| !probe.reachable && probe.detail.is_none()));
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
    assert!(matches!(
        sonarr(&fake).register_download_client(&sabnzbd()).await,
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

#[tokio::test]
async fn the_download_clients_are_read_back_by_their_endpoint() {
    // Servarr carries the connection settings as named entries in a `fields`
    // array, not top-level keys; the endpoint is decoded from there.
    let fake = Fake::new(Answer::Reply(
        200,
        r#"[{"id":3,"name":"SABnzbd","fields":[{"name":"host","value":"sabnzbd"},{"name":"port","value":8080},{"name":"tvCategory","value":"tv"}]}]"#,
    ));
    let clients = sonarr(&fake).download_clients().await;
    assert_eq!(
        clients.ok(),
        Some(vec![RegisteredClient {
            id: "3".to_owned(),
            host: "sabnzbd".to_owned(),
            port: 8080,
            // The category the client files under is read back too, so a later
            // run can tell an operator's re-filing from a fresh wire.
            category: Some(Category {
                field: "tvCategory".to_owned(),
                value: "tv".to_owned(),
            }),
        }])
    );

    let sent = fake.request();
    assert!(sent.is_some_and(|request| request.url.ends_with("/api/v3/downloadclient")));
}

#[tokio::test]
async fn a_client_that_names_no_endpoint_is_left_out_rather_than_guessed() {
    // A resource without both a host and a port cannot be matched by connection,
    // so it is left out rather than returned as an unusable half-endpoint.
    let fake = Fake::new(Answer::Reply(
        200,
        r#"[{"id":3,"fields":[{"name":"host","value":"sabnzbd"},{"name":"port","value":8080}]},{"id":4,"fields":[{"name":"port","value":9090}]}]"#,
    ));
    let clients = sonarr(&fake)
        .download_clients()
        .await
        .ok()
        .unwrap_or_default();
    assert_eq!(clients.len(), 1, "the half-specified client is left out");
    assert!(clients
        .iter()
        .any(|client| client.id == "3" && client.host == "sabnzbd" && client.port == 8080));
}

#[tokio::test]
async fn a_download_client_listing_that_is_refused_is_unauthorised() {
    let fake = Fake::new(Answer::Reply(401, ""));
    assert!(matches!(
        sonarr(&fake).download_clients().await,
        Err(Failure::Unauthorised { .. })
    ));
}

#[tokio::test]
async fn a_download_client_listing_with_no_answer_is_unavailable() {
    let fake = Fake::new(Answer::Silent);
    assert!(matches!(
        sonarr(&fake).download_clients().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn an_unreadable_download_client_list_is_refused() {
    let fake = Fake::new(Answer::Reply(200, "not an array"));
    assert!(matches!(
        sonarr(&fake).download_clients().await,
        Err(Failure::Refused { .. })
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

/// A client at the given API version over the fake.
fn versioned(fake: &Arc<Fake>, version: u32) -> Servarr {
    let http: Arc<dyn Http> = fake.clone();
    Servarr::new(http, "http://arr:8989", "the-key", "arr", version)
}

#[tokio::test]
async fn the_api_version_selects_the_path_segment() {
    // The one shape spans two versions: Lidarr answers at v1, Sonarr at v3, so the
    // version the manifest carries — not the client — decides the path.
    let folder = RootFolder {
        path: "/data/media/music".to_owned(),
        media_type: "music".to_owned(),
    };

    let lidarr = Fake::new(Answer::Reply(201, ""));
    assert!(versioned(&lidarr, 1)
        .register_root_folder(&folder)
        .await
        .is_ok());
    assert!(lidarr
        .request()
        .is_some_and(|request| request.url.ends_with("/api/v1/rootfolder")));

    let sonarr = Fake::new(Answer::Reply(201, ""));
    assert!(versioned(&sonarr, 3)
        .register_root_folder(&folder)
        .await
        .is_ok());
    assert!(sonarr
        .request()
        .is_some_and(|request| request.url.ends_with("/api/v3/rootfolder")));
}

#[tokio::test]
async fn a_json_post_declares_its_content_type() {
    // A body Servarr reads as JSON is announced as such; a service that binds by
    // content type would otherwise drop it.
    let fake = Fake::new(Answer::Reply(201, ""));
    let folder = RootFolder {
        path: "/data/media/tv".to_owned(),
        media_type: "tv".to_owned(),
    };
    assert!(sonarr(&fake).register_root_folder(&folder).await.is_ok());
    assert!(fake.request().is_some_and(|request| request
        .headers
        .iter()
        .any(|(name, value)| name == "Content-Type" && value == "application/json")));
}

#[tokio::test]
async fn a_get_carries_no_content_type() {
    // A read has no body, so it declares no content type.
    let fake = Fake::new(Answer::Reply(200, "[]"));
    let _ = sonarr(&fake).root_folders().await;
    assert!(fake.request().is_some_and(|request| request
        .headers
        .iter()
        .all(|(name, _)| name != "Content-Type")));
}

#[tokio::test]
async fn a_queue_item_carries_what_the_service_said_went_wrong() {
    // The blocking cause in the words of the thing that refused. A permission
    // denial from an import log is worth more than any interpretation of it, and
    // it is the difference between "stuck" and something an operator can fix.
    let fake = Fake::new(Answer::Reply(
        200,
        r#"{"totalRecords":1,"records":[{"title":"Some.Release","trackedDownloadStatus":"warning",
           "trackedDownloadState":"importPending","downloadId":"ABC123",
           "statusMessages":[{"messages":["Permission denied writing to /data/media"]}]}]}"#,
    ));
    let read = sonarr(&fake).queue().await.ok().unwrap_or_default();
    let first = read.items.first().cloned().unwrap_or_else(|| Queued {
        title: String::new(),
        status: String::new(),
        state: String::new(),
        message: None,
        download_id: None,
    });
    assert_eq!(first.title, "Some.Release");
    assert_eq!(first.state, "importPending");
    assert_eq!(
        first.message.as_deref(),
        Some("Permission denied writing to /data/media")
    );
    assert_eq!(first.download_id.as_deref(), Some("ABC123"));
}

#[tokio::test]
async fn a_service_that_offers_only_blank_detail_carries_none_rather_than_empty() {
    // An empty string is not a cause. Carrying one would put a blank line where an
    // explanation belongs, which reads as though the service explained itself.
    let fake = Fake::new(Answer::Reply(
        200,
        r#"{"totalRecords":1,"records":[{"title":"Some.Release","trackedDownloadStatus":"warning",
           "errorMessage":"   ","downloadId":""}]}"#,
    ));
    let read = sonarr(&fake).queue().await.ok().unwrap_or_default();
    let carried: Vec<(Option<String>, Option<String>)> = read
        .items
        .iter()
        .map(|item| (item.message.clone(), item.download_id.clone()))
        .collect();
    assert_eq!(carried, vec![(None, None)]);
}

#[tokio::test]
async fn the_queue_depth_and_the_stuck_count_are_read() {
    let fake = Fake::new(Answer::Reply(
        200,
        r#"{"totalRecords":5,"records":[{"trackedDownloadStatus":"ok"},{"trackedDownloadStatus":"warning"},{"trackedDownloadStatus":"Error"}]}"#,
    ));
    let queue = sonarr(&fake).queue().await;
    let depth = queue.as_ref().map(QueueDepth::of);
    assert_eq!(depth.ok(), Some(QueueDepth { total: 5, stuck: 2 }));

    // It asked the queue route, and for a generous page so the count is whole.
    assert!(fake
        .request()
        .is_some_and(|request| request.url.contains("/api/v3/queue?pageSize=")));
}

#[tokio::test]
async fn a_queue_that_is_not_answered_is_unavailable() {
    let fake = Fake::new(Answer::Silent);
    assert!(matches!(
        sonarr(&fake).queue().await,
        Err(Failure::Unavailable { .. })
    ));
}

// ---- Lidarr music-quality apply ----

/// A transport that answers by matching a request's method and a substring of its URL,
/// recording every request so a test can assert what a whole exchange sent.
struct Router {
    routes: Vec<(Method, &'static str, u16, String)>,
    seen: Mutex<Vec<Request>>,
}

impl Router {
    fn new(routes: Vec<(Method, &'static str, u16, String)>) -> Arc<Self> {
        Arc::new(Self {
            routes,
            seen: Mutex::new(Vec::new()),
        })
    }

    /// Every request the client sent, in order.
    fn sent(&self) -> Vec<Request> {
        self.seen
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Http for Router {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        if let Ok(mut guard) = self.seen.lock() {
            guard.push(request.clone());
        }
        for (method, needle, status, body) in &self.routes {
            if request.method == *method && request.url.contains(needle) {
                return Ok(Response {
                    status: *status,
                    body: body.clone(),
                });
            }
        }
        Ok(Response {
            status: 404,
            body: String::new(),
        })
    }
}

/// A Lidarr client over the given router — the v1 Lidarr answers at.
fn lidarr(router: &Arc<Router>) -> Servarr {
    let http: Arc<dyn Http> = router.clone();
    Servarr::new(http, "http://lidarr:8686", "the-key", "lidarr", 1)
}

/// A profile list a GET returns: a stray non-object (skipped), an object with no id
/// (rewritten but not addressable, so not sent), and a full profile that is updated —
/// carrying the 24-bit format in its items so a hi-res choice has something to score.
const PROFILES: &str = r#"[
    1,
    {"upgradeAllowed":false,"cutoff":1006,"items":[
        {"id":1005,"name":"High Quality Lossy","allowed":false,"items":[]},
        {"id":1006,"name":"Lossless","allowed":false,"items":[]}
    ]},
    {"id":2,"upgradeAllowed":false,"cutoff":1006,"cutoffFormatScore":0,
     "formatItems":[{"format":9,"name":"lemonfiber: 24-bit","score":0}],
     "items":[
        {"id":1005,"name":"High Quality Lossy","allowed":false,"items":[]},
        {"id":1006,"name":"Lossless","allowed":false,"items":[]}
    ]}
]"#;

#[tokio::test]
async fn a_lossless_choice_updates_each_addressable_profile() {
    let router = Router::new(vec![
        (Method::Get, "/qualityprofile", 200, PROFILES.to_owned()),
        (Method::Put, "/qualityprofile", 200, String::new()),
    ]);
    let applied = lidarr(&router).apply_music_format(Format::Lossless).await;
    assert!(applied.is_ok());

    // Only the profile that carries an id is addressed — the stray and the id-less one
    // are passed over rather than sent nowhere.
    let puts: Vec<Request> = router
        .sent()
        .into_iter()
        .filter(|request| request.method == Method::Put)
        .collect();
    assert_eq!(puts.len(), 1);
    let put = puts.first();
    assert!(put.is_some_and(|request| request.url.ends_with("/api/v1/qualityprofile/2")));
    assert!(put
        .and_then(|request| request.body.as_deref())
        .is_some_and(|body| body.contains(r#""upgradeAllowed":true"#)));

    // A non-hi-res choice touches no custom format.
    assert!(!router
        .sent()
        .iter()
        .any(|request| request.url.contains("/customformat")));
}

#[tokio::test]
async fn a_compact_choice_updates_the_profile_without_touching_a_custom_format() {
    let router = Router::new(vec![
        (Method::Get, "/qualityprofile", 200, PROFILES.to_owned()),
        (Method::Put, "/qualityprofile", 200, String::new()),
    ]);
    let applied = lidarr(&router).apply_music_format(Format::Compact).await;
    assert!(applied.is_ok());
    // Compact addresses the profile but, like every non-hi-res choice, leaves custom
    // formats alone.
    assert!(router
        .sent()
        .iter()
        .any(|request| request.method == Method::Put));
    assert!(!router
        .sent()
        .iter()
        .any(|request| request.url.contains("/customformat")));
}

#[tokio::test]
async fn a_hi_res_choice_creates_the_24_bit_format_and_prefers_it() {
    let router = Router::new(vec![
        (Method::Get, "/customformat", 200, "[]".to_owned()),
        (Method::Post, "/customformat", 201, String::new()),
        (Method::Get, "/qualityprofile", 200, PROFILES.to_owned()),
        (Method::Put, "/qualityprofile", 200, String::new()),
    ]);
    let applied = lidarr(&router).apply_music_format(Format::HiRes).await;
    assert!(applied.is_ok());

    // The 24-bit format is created, as a release-title match.
    let posts: Vec<Request> = router
        .sent()
        .into_iter()
        .filter(|request| request.method == Method::Post && request.url.contains("/customformat"))
        .collect();
    assert_eq!(posts.len(), 1);
    assert!(posts
        .first()
        .and_then(|request| request.body.as_deref())
        .is_some_and(|body| body.contains("ReleaseTitleSpecification")));

    // The profile update prefers it, through a positive cutoff format score.
    let put = router
        .sent()
        .into_iter()
        .find(|request| request.method == Method::Put);
    assert!(put
        .and_then(|request| request.body)
        .is_some_and(|body| body.contains(r#""cutoffFormatScore":100"#)));
}

#[tokio::test]
async fn an_existing_24_bit_format_is_not_created_again() {
    let router = Router::new(vec![
        (
            Method::Get,
            "/customformat",
            200,
            r#"[{"id":9,"name":"lemonfiber: 24-bit"}]"#.to_owned(),
        ),
        (Method::Get, "/qualityprofile", 200, PROFILES.to_owned()),
        (Method::Put, "/qualityprofile", 200, String::new()),
    ]);
    let applied = lidarr(&router).apply_music_format(Format::HiRes).await;
    assert!(applied.is_ok());
    assert!(
        !router
            .sent()
            .iter()
            .any(|request| request.method == Method::Post),
        "the format already existed, so it was not created again"
    );
}

#[tokio::test]
async fn an_unreadable_profile_list_is_a_failure() {
    let router = Router::new(vec![(
        Method::Get,
        "/qualityprofile",
        200,
        "not json".to_owned(),
    )]);
    assert!(lidarr(&router)
        .apply_music_format(Format::Lossless)
        .await
        .is_err());
}

#[tokio::test]
async fn a_refused_profile_update_is_a_failure() {
    let router = Router::new(vec![
        (Method::Get, "/qualityprofile", 200, PROFILES.to_owned()),
        (Method::Put, "/qualityprofile", 500, "boom".to_owned()),
    ]);
    assert!(lidarr(&router)
        .apply_music_format(Format::Lossless)
        .await
        .is_err());
}

#[tokio::test]
async fn an_unreadable_custom_format_list_is_a_failure() {
    let router = Router::new(vec![(
        Method::Get,
        "/customformat",
        200,
        "not json".to_owned(),
    )]);
    assert!(lidarr(&router)
        .apply_music_format(Format::HiRes)
        .await
        .is_err());
}

#[tokio::test]
async fn a_refused_custom_format_creation_is_a_failure() {
    let router = Router::new(vec![
        (Method::Get, "/customformat", 200, "[]".to_owned()),
        (Method::Post, "/customformat", 500, "nope".to_owned()),
    ]);
    assert!(lidarr(&router)
        .apply_music_format(Format::HiRes)
        .await
        .is_err());
}

// ---- Pipeline (item trace fragment) ----

/// A Sonarr client (v3) over the given router.
fn sonarr_routed(router: &Arc<Router>) -> Servarr {
    let http: Arc<dyn Http> = router.clone();
    Servarr::new(http, "http://sonarr:8989", "the-key", "sonarr", 3)
}

#[tokio::test]
async fn find_items_matches_the_library_by_human_title() {
    let router = Router::new(vec![(
        Method::Get,
        "/series",
        200,
        r#"[{"id":1,"title":"The Expanse","monitored":true},
            {"id":2,"title":"Foundation","monitored":false}]"#
            .to_owned(),
    )]);
    // Case-insensitive substring of the title, never an internal id.
    let found = sonarr_routed(&router)
        .find_items(Kind::Sonarr, "expanse")
        .await
        .unwrap_or_default();
    assert_eq!(found.len(), 1);
    let item = found.first();
    assert_eq!(item.map(|i| i.id), Some(1));
    assert_eq!(item.map(|i| i.title.as_str()), Some("The Expanse"));
    assert_eq!(item.map(|i| i.monitored), Some(true));
}

#[tokio::test]
async fn find_items_reads_the_library_for_the_service_kind() {
    // Radarr's library is movies, not series — the endpoint follows the kind.
    let router = Router::new(vec![(
        Method::Get,
        "/movie",
        200,
        r#"[{"id":7,"title":"Dune","monitored":true}]"#.to_owned(),
    )]);
    let radarr = {
        let http: Arc<dyn Http> = router.clone();
        Servarr::new(http, "http://radarr:7878", "the-key", "radarr", 3)
    };
    let found = radarr
        .find_items(Kind::Radarr, "dune")
        .await
        .unwrap_or_default();
    assert_eq!(found.len(), 1);
    // The request went to the movie endpoint.
    assert!(router
        .sent()
        .iter()
        .any(|request| request.url.ends_with("/api/v3/movie")));
}

#[tokio::test]
async fn item_history_keeps_the_notable_events_and_drops_the_rest() {
    let router = Router::new(vec![(
        Method::Get,
        "/history",
        200,
        r#"{"records":[
            {"eventType":"downloadFolderImported","date":"2026-01-02T00:00:00Z","episodeId":42},
            {"eventType":"downloadFailed","date":"2026-01-01T12:00:00Z"},
            {"eventType":"grabbed","date":"2026-01-01T00:00:00Z","episodeId":42},
            {"eventType":"episodeFileRenamed","date":"2025-12-31T00:00:00Z"}
        ]}"#
        .to_owned(),
    )]);
    let events = sonarr_routed(&router)
        .item_history(Kind::Sonarr, 1)
        .await
        .unwrap_or_default();
    // The import, the failed download and the grab are all notable history — the failure
    // shows even though it advances no stage; the rename is not notable, so it is dropped.
    // Newest first.
    assert_eq!(events.len(), 3);
    assert_eq!(
        events.first().map(|event| event.outcome),
        Some(Outcome::Imported)
    );
    assert!(events
        .iter()
        .any(|event| event.outcome == Outcome::DownloadFailed));
    assert!(events.iter().any(|event| event.outcome == Outcome::Grabbed));
    // Each event names the episode it happened to, where the service records one — the
    // only proof a trace has that a particular episode was ever grabbed, since the
    // episode listing's own grabbed flag is never populated.
    assert_eq!(
        events.first().and_then(|event| event.part),
        Some(42),
        "the episode a history event names is carried through"
    );
    assert!(events
        .iter()
        .any(|event| event.outcome == Outcome::DownloadFailed && event.part.is_none()));
    // It filtered by the item, on the kind's own history parameter.
    assert!(router
        .sent()
        .iter()
        .any(|request| request.url.contains("seriesIds=1")));
}

#[tokio::test]
async fn an_unreadable_library_is_a_failure() {
    let router = Router::new(vec![(Method::Get, "/series", 200, "not json".to_owned())]);
    assert!(sonarr_routed(&router)
        .find_items(Kind::Sonarr, "x")
        .await
        .is_err());
}

#[tokio::test]
async fn an_unreadable_history_is_a_failure() {
    let router = Router::new(vec![(Method::Get, "/history", 200, "not json".to_owned())]);
    assert!(sonarr_routed(&router)
        .item_history(Kind::Sonarr, 1)
        .await
        .is_err());
}

#[tokio::test]
async fn item_queue_reads_a_downloading_item_by_series() {
    let router = Router::new(vec![(
        Method::Get,
        "/queue",
        200,
        r#"{"records":[
            {"seriesId":1,"episodeId":42,"trackedDownloadState":"downloading","trackedDownloadStatus":"ok"}
        ]}"#
        .to_owned(),
    )]);
    let queue = sonarr_routed(&router)
        .item_queue(Kind::Sonarr, 1)
        .await
        .unwrap_or_default();
    // The record names the episode it is for, so a series' queue can be read per episode
    // rather than flattened to one state for the whole show.
    assert_eq!(
        queue,
        vec![QueueItem {
            part: Some(42),
            stage: Stage::Downloading,
            stuck: false
        }]
    );
}

#[tokio::test]
async fn item_queue_reads_a_film_by_movie_and_flags_stuck() {
    // Radarr matches on movieId; a warning tracked status is stuck, and an unrecognised
    // state still counts as at least downloading.
    let router = Router::new(vec![(
        Method::Get,
        "/queue",
        200,
        r#"{"records":[
            {"movieId":7,"trackedDownloadState":"stalled","trackedDownloadStatus":"warning"}
        ]}"#
        .to_owned(),
    )]);
    let radarr = {
        let http: Arc<dyn Http> = router.clone();
        Servarr::new(http, "http://radarr:7878", "the-key", "radarr", 3)
    };
    let queue = radarr.item_queue(Kind::Radarr, 7).await.unwrap_or_default();
    // A film's record names no part — the record is for the whole item.
    assert_eq!(
        queue,
        vec![QueueItem {
            part: None,
            stage: Stage::Downloading,
            stuck: true
        }]
    );
}

#[tokio::test]
async fn item_queue_walks_past_the_first_page_to_find_the_item() {
    // A full first page of other items, and a total beyond it: the traced item sits on
    // page two, so reading only the first page would miss it and misreport it as stuck at
    // grabbed. Two of its records are there at different states, so the furthest shows.
    // The 200 matches the client's page size, so the first page is full and it reads on.
    let filler = vec![
        r#"{"seriesId":99,"trackedDownloadState":"downloading","trackedDownloadStatus":"ok"}"#;
        200
    ]
    .join(",");
    let page_one = format!(r#"{{"totalRecords":201,"records":[{filler}]}}"#);
    let page_two = r#"{"totalRecords":201,"records":[
        {"seriesId":1,"trackedDownloadState":"downloading","trackedDownloadStatus":"ok"},
        {"seriesId":1,"trackedDownloadState":"importPending","trackedDownloadStatus":"ok"}
    ]}"#
    .to_owned();
    let router = Router::new(vec![
        (Method::Get, "/queue?page=1", 200, page_one),
        (Method::Get, "/queue?page=2", 200, page_two),
    ]);
    let queue = sonarr_routed(&router)
        .item_queue(Kind::Sonarr, 1)
        .await
        .unwrap_or_default();
    // Both of the item's records come back, so the furthest is the caller's to take —
    // which for the item as a whole is the import pending on page two.
    assert_eq!(
        queue.iter().map(|record| record.stage).max(),
        Some(Stage::Downloaded)
    );
    assert_eq!(queue.len(), 2);
    // The walk did not stop at the first page.
    assert!(router
        .sent()
        .iter()
        .any(|request| request.url.contains("page=2")));
}

#[tokio::test]
async fn item_queue_holding_nothing_for_the_item_is_empty() {
    let router = Router::new(vec![(
        Method::Get,
        "/queue",
        200,
        r#"{"records":[{"seriesId":99,"trackedDownloadState":"downloading"}]}"#.to_owned(),
    )]);
    let queue = sonarr_routed(&router)
        .item_queue(Kind::Sonarr, 1)
        .await
        .unwrap_or_default();
    assert!(queue.is_empty());
}

#[tokio::test]
async fn item_parts_reads_the_episodes_of_a_series() {
    let router = Router::new(vec![(
        Method::Get,
        "/episode",
        200,
        r#"[
            {"id":11,"seasonNumber":1,"episodeNumber":1,"title":"Dulcinea","monitored":true,"hasFile":true},
            {"id":12,"seasonNumber":1,"episodeNumber":2,"title":"The Big Empty","monitored":true,"hasFile":false},
            {"id":13,"seasonNumber":0,"episodeNumber":1,"title":"A Special"}
        ]"#
        .to_owned(),
    )]);
    let parts = sonarr_routed(&router)
        .item_parts(Kind::Sonarr, 1, None)
        .await
        .unwrap_or_default();
    let read: Vec<(i64, u32, u32, &str, bool, bool)> = parts
        .iter()
        .map(|part| {
            (
                part.id,
                part.season,
                part.number,
                part.title.as_str(),
                part.monitored,
                part.has_file,
            )
        })
        .collect();
    // The third record carries none of the flags: it reads as unmonitored and absent
    // rather than failing the whole read.
    assert_eq!(
        read,
        vec![
            (11, 1, 1, "Dulcinea", true, true),
            (12, 1, 2, "The Big Empty", true, false),
            (13, 0, 1, "A Special", false, false),
        ]
    );
    // The listing was narrowed to the one series asked about.
    assert!(router
        .sent()
        .iter()
        .any(|request| request.url.contains("seriesId=1")));
}

#[tokio::test]
async fn item_parts_narrows_to_one_season_at_the_service() {
    let router = Router::new(vec![(Method::Get, "/episode", 200, "[]".to_owned())]);
    let parts = sonarr_routed(&router)
        .item_parts(Kind::Sonarr, 1, Some(2))
        .await
        .unwrap_or_default();
    assert!(parts.is_empty());
    // The season filter is the service's own, not a slice taken after reading them all.
    assert!(router
        .sent()
        .iter()
        .any(|request| request.url.contains("seasonNumber=2")));
}

#[tokio::test]
async fn a_film_has_no_parts_and_is_never_asked_for_them() {
    // A film is the whole item. Asking a service that files nothing per part would be a
    // request with no answer, so none is made.
    let router = Router::new(Vec::new());
    let radarr = {
        let http: Arc<dyn Http> = router.clone();
        Servarr::new(http, "http://radarr:7878", "the-key", "radarr", 3)
    };
    let parts = radarr
        .item_parts(Kind::Radarr, 7, None)
        .await
        .unwrap_or_default();
    assert!(parts.is_empty());
    assert!(router.sent().is_empty());
}

#[tokio::test]
async fn unreadable_episodes_are_a_failure() {
    let router = Router::new(vec![(Method::Get, "/episode", 200, "not json".to_owned())]);
    assert!(sonarr_routed(&router)
        .item_parts(Kind::Sonarr, 1, None)
        .await
        .is_err());
}

#[tokio::test]
async fn stuck_items_names_each_stuck_show_once() {
    // Five queued records: two stuck episodes of one show, a healthy one, a stuck one whose
    // embedded title is empty, and a stuck one with no show at all.
    let router = Router::new(vec![(
        Method::Get,
        "/queue",
        200,
        r#"{"records":[
            {"trackedDownloadStatus":"warning","trackedDownloadState":"downloading","series":{"title":"The Expanse"}},
            {"trackedDownloadStatus":"error","trackedDownloadState":"importPending","series":{"title":"The Expanse"}},
            {"trackedDownloadStatus":"ok","trackedDownloadState":"downloading","series":{"title":"Not Stuck"}},
            {"trackedDownloadStatus":"warning","trackedDownloadState":"downloading","series":{"title":""}},
            {"trackedDownloadStatus":"warning","trackedDownloadState":"downloading"}
        ]}"#
        .to_owned(),
    )]);
    let items = sonarr_routed(&router)
        .stuck_items(Kind::Sonarr)
        .await
        .unwrap_or_default();
    // The show is listed once though two of its episodes are stuck; the healthy one is
    // excluded, and the two with nothing to trace by — an empty title and no title — are
    // left out rather than linked to a search that could not find them.
    let named: Vec<(&str, Stage)> = items
        .iter()
        .map(|item| (item.title.as_str(), item.stage))
        .collect();
    assert_eq!(named, vec![("The Expanse", Stage::Downloading)]);
    // The queue was read with the item included so each could be named.
    assert!(router
        .sent()
        .iter()
        .any(|request| request.url.contains("includeSeries=true")));
}

#[tokio::test]
async fn stuck_items_names_a_stuck_film_by_its_movie() {
    let router = Router::new(vec![(
        Method::Get,
        "/queue",
        200,
        r#"{"records":[{"trackedDownloadStatus":"error","trackedDownloadState":"downloading","movie":{"title":"Dune"}}]}"#
            .to_owned(),
    )]);
    let radarr = {
        let http: Arc<dyn Http> = router.clone();
        Servarr::new(http, "http://radarr:7878", "the-key", "radarr", 3)
    };
    let items = radarr.stuck_items(Kind::Radarr).await.unwrap_or_default();
    assert_eq!(items.first().map(|item| item.title.as_str()), Some("Dune"));
    assert!(router
        .sent()
        .iter()
        .any(|request| request.url.contains("includeMovie=true")));
}

#[tokio::test]
async fn an_unreadable_queue_is_a_failure() {
    let router = Router::new(vec![(Method::Get, "/queue", 200, "not json".to_owned())]);
    assert!(sonarr_routed(&router)
        .item_queue(Kind::Sonarr, 1)
        .await
        .is_err());
}

#[tokio::test]
async fn whether_the_service_hardlinks_is_read_from_its_own_settings() {
    let fake = Fake::new(Answer::Reply(200, r#"{"id":1,"copyUsingHardlinks":true}"#));
    assert_eq!(sonarr(&fake).hardlinks().await.ok(), Some(true));
}

#[tokio::test]
async fn telling_it_to_copy_keeps_every_other_setting_it_had() {
    // The service replaces the whole document on a write, so sending only the one
    // field would silently reset settings the operator chose themselves.
    let fake = Fake::new(Answer::Reply(
        200,
        r#"{"id":3,"copyUsingHardlinks":true,"importExtraFiles":true,"recycleBin":"/data/bin"}"#,
    ));
    assert!(sonarr(&fake).set_hardlinks(false).await.is_ok());

    let sent = fake
        .request()
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(sent.contains(r#""copyUsingHardlinks":false"#), "{sent}");
    assert!(sent.contains(r#""importExtraFiles":true"#), "kept: {sent}");
    assert!(sent.contains(r#""recycleBin":"/data/bin""#), "kept: {sent}");
    assert!(
        fake.request()
            .is_some_and(|request| request.url.ends_with("/config/mediamanagement/3")),
        "written back to its own id"
    );
}

#[tokio::test]
async fn settings_that_are_not_an_object_are_refused_rather_than_guessed() {
    let fake = Fake::new(Answer::Reply(200, "[]"));
    assert!(sonarr(&fake).set_hardlinks(false).await.is_err());
}
