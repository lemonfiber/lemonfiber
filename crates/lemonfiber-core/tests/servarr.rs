//! The Servarr client, driven through the HTTP port against a fake transport.
//!
//! The client turns a request into an API call and reads what the service
//! answered; the fake is that service, replying with exactly the status and body
//! a test wants — so every branch of the identity and registration paths is
//! exercised with nothing running. The client speaks an async trait built on
//! another, so it is driven from here rather than from an in-crate test, where it
//! would be compiled twice and its coverage counted from the wrong copy.

use std::sync::Arc;

use lemonfiber_core::audio::Format;
use lemonfiber_core::ports::http::{Http, Method, Request};
use lemonfiber_core::ports::service::{
    Category, Client, ClientKind, Credential, DownloadClient, Failure, Maintenance, MusicQuality,
    QueueDepth, Queued, Queues, RegisteredClient, RegisteredFolder, RootFolder,
};
use lemonfiber_core::servarr::{api_key, Servarr};
use lemonfiber_fixtures::http::{Answer, Fake};

/// A Sonarr client over the given fake — the v3 the media *arrs answer at.
fn sonarr(fake: &Arc<Fake>) -> Servarr {
    let http: Arc<dyn Http> = fake.clone();
    Servarr::new(http, "http://sonarr:8989", "the-key", "sonarr", 3)
}

#[tokio::test]
async fn a_valid_credential_reads_the_service_identity() {
    let fake = Fake::always(Answer::reply(
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
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"appName":"Radarr","version":"5.0"}"#,
    ));
    let identity = sonarr(&fake).identity().await;
    assert_eq!(identity.ok().map(|who| who.name), Some("Radarr".to_owned()));
}

#[tokio::test]
async fn a_command_is_posted_by_name_and_accepted() {
    let fake = Fake::always(Answer::reply(201, r#"{"name":"CutoffUnmetEpisodeSearch"}"#));
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
    let fake = Fake::always(Answer::reply(500, "boom"));
    assert!(sonarr(&fake)
        .run_command("CutoffUnmetEpisodeSearch")
        .await
        .is_err());
}

#[tokio::test]
async fn a_rejected_key_is_unauthorised() {
    let fake = Fake::always(Answer::reply(401, ""));
    assert!(matches!(
        sonarr(&fake).identity().await,
        Err(Failure::Unauthorised { .. })
    ));
}

#[tokio::test]
async fn a_service_that_is_not_answering_is_unavailable() {
    let fake = Fake::always(Answer::Silent);
    assert!(matches!(
        sonarr(&fake).identity().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn an_unexpected_status_is_refused_with_the_services_own_words() {
    // The service's own message is carried through, not paraphrased.
    let fake = Fake::always(Answer::reply(500, "database is locked"));
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
    let fake = Fake::always(Answer::reply(503, ""));
    let detail = match sonarr(&fake).identity().await {
        Err(Failure::Refused { detail, .. }) => Some(detail),
        _ => None,
    };
    assert_eq!(detail.as_deref(), Some("HTTP 503"));
}

#[tokio::test]
async fn an_unreadable_status_body_is_refused_and_the_detail_names_the_break() {
    let fake = Fake::always(Answer::reply(200, "not json at all"));
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
    let fake = Fake::always(Answer::reply(200, r#"{"version":"4.0"}"#));
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
    let fake = Fake::always(Answer::reply(404, ""));
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
    let fake = Fake::always(Answer::reply(404, ""));
    assert!(matches!(
        sonarr(&fake).root_folders().await,
        Err(Failure::Unsupported { .. })
    ));
}

#[tokio::test]
async fn a_root_folder_is_posted_to_its_endpoint() {
    let fake = Fake::always(Answer::reply(201, ""));
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
    let fake = Fake::always(Answer::reply(201, ""));
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
    let fake = Fake::always(Answer::reply(201, ""));
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
    let fake = Fake::always(Answer::reply(201, ""));
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
    let fake = Fake::always(Answer::reply(200, ""));
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

/// Putting one field back reads the client, changes that field, and writes the rest of the
/// document exactly as the service gave it — which is what lets a reversal restore a
/// category without ever having held the client's credential.
#[tokio::test]
async fn one_field_is_put_back_leaving_the_rest_of_the_client_alone() {
    let held = r#"{"id":7,"name":"SABnzbd","fields":[{"name":"host","value":"sabnzbd"},
        {"name":"apiKey","value":"kept"},{"name":"tvCategory","value":"mine"}]}"#;
    let fake = Fake::in_turn(vec![
        Answer::reply(200, held),
        Answer::reply(200, String::new()),
    ]);

    assert!(sonarr(&fake)
        .set_client_field("7", "tvCategory", Some("tv-sonarr"))
        .await
        .is_ok());

    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|request| request.method == Method::Put
            && request.url.ends_with("/api/v3/downloadclient/7")));
    let body = sent.and_then(|request| request.body).unwrap_or_default();
    assert!(body.contains("tv-sonarr"), "{body}");
    // Everything the reversal never knew about goes back untouched, credential included.
    assert!(body.contains("kept"), "{body}");
    assert!(!body.contains("mine"), "{body}");
}

/// A field that held nothing before is taken out rather than set to the empty string,
/// which a service would read as a value somebody chose.
#[tokio::test]
async fn a_field_put_back_to_nothing_is_taken_out() {
    let held = r#"{"id":7,"fields":[{"name":"host","value":"sabnzbd"},
        {"name":"tvCategory","value":"mine"}]}"#;
    let fake = Fake::in_turn(vec![
        Answer::reply(200, held),
        Answer::reply(200, String::new()),
    ]);

    assert!(sonarr(&fake)
        .set_client_field("7", "tvCategory", None)
        .await
        .is_ok());

    let body = fake
        .request()
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(!body.contains("tvCategory"), "{body}");
}

/// A field the client does not carry is added, so a reversal can put back a category the
/// service dropped rather than reporting success and changing nothing.
#[tokio::test]
async fn a_field_the_client_does_not_carry_is_added() {
    let held = r#"{"id":7,"fields":[{"name":"host","value":"sabnzbd"}]}"#;
    let fake = Fake::in_turn(vec![
        Answer::reply(200, held),
        Answer::reply(200, String::new()),
    ]);

    assert!(sonarr(&fake)
        .set_client_field("7", "tvCategory", Some("tv-sonarr"))
        .await
        .is_ok());

    let body = fake
        .request()
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(body.contains("tv-sonarr"), "{body}");
}

/// A client whose document carries no settings at all is refused rather than written to on
/// a guess about a shape this build does not recognise.
#[tokio::test]
async fn a_client_with_no_settings_to_put_back_is_refused() {
    let fake = Fake::always(Answer::reply(200, r#"{"id":7}"#));

    assert!(matches!(
        sonarr(&fake)
            .set_client_field("7", "tvCategory", None)
            .await,
        Err(Failure::Refused { .. })
    ));
}

/// A service that will not hand the client over cannot have one field put back, and says
/// so rather than writing a document it never read.
#[tokio::test]
async fn a_client_that_cannot_be_read_is_not_written_back() {
    let fake = Fake::always(Answer::reply(500, "boom"));

    assert!(sonarr(&fake)
        .set_client_field("7", "tvCategory", None)
        .await
        .is_err());
}

#[tokio::test]
async fn an_update_with_an_id_the_service_did_not_assign_is_refused() {
    // A non-numeric id is not one a Servarr service assigns, so there is nothing to
    // address — refused rather than a malformed request sent.
    let fake = Fake::always(Answer::reply(200, ""));
    assert!(matches!(
        sonarr(&fake)
            .update_download_client("not-a-number", &sabnzbd())
            .await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_rejected_download_client_update_is_refused() {
    let fake = Fake::always(Answer::reply(400, "cannot update"));
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
    let fake = Fake::always(Answer::reply(200, body));
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
    let fake = Fake::always(Answer::reply(400, "path already used"));
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
    let fake = Fake::always(Answer::reply(400, "unknown implementation"));
    assert!(matches!(
        sonarr(&fake).register_download_client(&sabnzbd()).await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn the_root_folders_are_read_back_with_their_ids() {
    let fake = Fake::always(Answer::reply(
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
    let fake = Fake::always(Answer::reply(200, "not an array"));
    assert!(matches!(
        sonarr(&fake).root_folders().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_rejected_folder_listing_is_unauthorised() {
    let fake = Fake::always(Answer::reply(401, ""));
    assert!(matches!(
        sonarr(&fake).root_folders().await,
        Err(Failure::Unauthorised { .. })
    ));
}

#[tokio::test]
async fn a_folder_listing_with_no_answer_is_unavailable() {
    let fake = Fake::always(Answer::Silent);
    assert!(matches!(
        sonarr(&fake).root_folders().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn the_download_clients_are_read_back_by_their_endpoint() {
    // Servarr carries the connection settings as named entries in a `fields`
    // array, not top-level keys; the endpoint is decoded from there.
    let fake = Fake::always(Answer::reply(
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
    let fake = Fake::always(Answer::reply(
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
    let fake = Fake::always(Answer::reply(401, ""));
    assert!(matches!(
        sonarr(&fake).download_clients().await,
        Err(Failure::Unauthorised { .. })
    ));
}

#[tokio::test]
async fn a_download_client_listing_with_no_answer_is_unavailable() {
    let fake = Fake::always(Answer::Silent);
    assert!(matches!(
        sonarr(&fake).download_clients().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn an_unreadable_download_client_list_is_refused() {
    let fake = Fake::always(Answer::reply(200, "not an array"));
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

    let lidarr = Fake::always(Answer::reply(201, ""));
    assert!(versioned(&lidarr, 1)
        .register_root_folder(&folder)
        .await
        .is_ok());
    assert!(lidarr
        .request()
        .is_some_and(|request| request.url.ends_with("/api/v1/rootfolder")));

    let sonarr = Fake::always(Answer::reply(201, ""));
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
    let fake = Fake::always(Answer::reply(201, ""));
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
    let fake = Fake::always(Answer::reply(200, "[]"));
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
    let fake = Fake::always(Answer::reply(
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
        grabs: 1,
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
    let fake = Fake::always(Answer::reply(
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
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"totalRecords":5,"records":[{"trackedDownloadStatus":"ok"},{"trackedDownloadStatus":"warning"},{"trackedDownloadStatus":"Error"}]}"#,
    ));
    let queue = sonarr(&fake).queue().await;
    let depth = queue.as_ref().map(QueueDepth::of);
    assert_eq!(depth.ok(), Some(QueueDepth { total: 5, stuck: 2 }));

    // It asked the queue route, and for a generous page so the count is whole.
    assert!(fake.asked_for("/api/v3/queue?pageSize="));
}

#[tokio::test]
async fn a_queue_that_is_not_answered_is_unavailable() {
    let fake = Fake::always(Answer::Silent);
    assert!(matches!(
        sonarr(&fake).queue().await,
        Err(Failure::Unavailable { .. })
    ));
}

// ---- Lidarr music-quality apply ----

/// A Lidarr client over the given router — the v1 Lidarr answers at.
fn lidarr(router: &Arc<Fake>) -> Servarr {
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
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/qualityprofile",
            Answer::reply(200, PROFILES.to_owned()),
        ),
        (
            Method::Put,
            "/qualityprofile",
            Answer::reply(200, String::new()),
        ),
    ]);
    let applied = lidarr(&router).apply_music_format(Format::Lossless).await;
    assert!(applied.is_ok());

    // Only the profile that carries an id is addressed — the stray and the id-less one
    // are passed over rather than sent nowhere.
    let puts: Vec<Request> = router
        .requests()
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
        .requests()
        .iter()
        .any(|request| request.url.contains("/customformat")));
}

#[tokio::test]
async fn a_compact_choice_updates_the_profile_without_touching_a_custom_format() {
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/qualityprofile",
            Answer::reply(200, PROFILES.to_owned()),
        ),
        (
            Method::Put,
            "/qualityprofile",
            Answer::reply(200, String::new()),
        ),
    ]);
    let applied = lidarr(&router).apply_music_format(Format::Compact).await;
    assert!(applied.is_ok());
    // Compact addresses the profile but, like every non-hi-res choice, leaves custom
    // formats alone.
    assert!(router
        .requests()
        .iter()
        .any(|request| request.method == Method::Put));
    assert!(!router
        .requests()
        .iter()
        .any(|request| request.url.contains("/customformat")));
}

#[tokio::test]
async fn a_hi_res_choice_creates_the_24_bit_format_and_prefers_it() {
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/customformat",
            Answer::reply(200, "[]".to_owned()),
        ),
        (
            Method::Post,
            "/customformat",
            Answer::reply(201, String::new()),
        ),
        (
            Method::Get,
            "/qualityprofile",
            Answer::reply(200, PROFILES.to_owned()),
        ),
        (
            Method::Put,
            "/qualityprofile",
            Answer::reply(200, String::new()),
        ),
    ]);
    let applied = lidarr(&router).apply_music_format(Format::HiRes).await;
    assert!(applied.is_ok());

    // The 24-bit format is created, as a release-title match.
    let posts: Vec<Request> = router
        .requests()
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
        .requests()
        .into_iter()
        .find(|request| request.method == Method::Put);
    assert!(put
        .and_then(|request| request.body)
        .is_some_and(|body| body.contains(r#""cutoffFormatScore":100"#)));
}

#[tokio::test]
async fn an_existing_24_bit_format_is_not_created_again() {
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/customformat",
            Answer::reply(200, r#"[{"id":9,"name":"lemonfiber: 24-bit"}]"#.to_owned()),
        ),
        (
            Method::Get,
            "/qualityprofile",
            Answer::reply(200, PROFILES.to_owned()),
        ),
        (
            Method::Put,
            "/qualityprofile",
            Answer::reply(200, String::new()),
        ),
    ]);
    let applied = lidarr(&router).apply_music_format(Format::HiRes).await;
    assert!(applied.is_ok());
    assert!(
        !router
            .requests()
            .iter()
            .any(|request| request.method == Method::Post),
        "the format already existed, so it was not created again"
    );
}

#[tokio::test]
async fn an_unreadable_profile_list_is_a_failure() {
    let router = Fake::by_route(vec![(
        Method::Get,
        "/qualityprofile",
        Answer::reply(200, "not json".to_owned()),
    )]);
    assert!(lidarr(&router)
        .apply_music_format(Format::Lossless)
        .await
        .is_err());
}

#[tokio::test]
async fn a_refused_profile_update_is_a_failure() {
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/qualityprofile",
            Answer::reply(200, PROFILES.to_owned()),
        ),
        (
            Method::Put,
            "/qualityprofile",
            Answer::reply(500, "boom".to_owned()),
        ),
    ]);
    assert!(lidarr(&router)
        .apply_music_format(Format::Lossless)
        .await
        .is_err());
}

#[tokio::test]
async fn an_unreadable_custom_format_list_is_a_failure() {
    let router = Fake::by_route(vec![(
        Method::Get,
        "/customformat",
        Answer::reply(200, "not json".to_owned()),
    )]);
    assert!(lidarr(&router)
        .apply_music_format(Format::HiRes)
        .await
        .is_err());
}

#[tokio::test]
async fn a_refused_custom_format_creation_is_a_failure() {
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/customformat",
            Answer::reply(200, "[]".to_owned()),
        ),
        (
            Method::Post,
            "/customformat",
            Answer::reply(500, "nope".to_owned()),
        ),
    ]);
    assert!(lidarr(&router)
        .apply_music_format(Format::HiRes)
        .await
        .is_err());
}

/// The \*arr that files by artist is given a root folder it will accept.
///
/// It refuses one described by path alone: it wants a name and the two profiles
/// anything found beneath the folder is fetched at. The ids are read from the service
/// rather than assumed, because they are numbered per installation.
#[tokio::test]
async fn a_music_root_folder_carries_the_name_and_profiles_that_service_requires() {
    let fake = Fake::by_path(vec![
        (
            "/metadataprofile",
            Answer::reply(200, r#"[{"id":7,"name":"Standard"}]"#),
        ),
        (
            "/qualityprofile",
            Answer::reply(200, r#"[{"id":4,"name":"Lossless"}]"#),
        ),
        ("/rootfolder", Answer::reply(201, "")),
    ]);
    let folder = RootFolder {
        path: "/data/media/music".to_owned(),
        media_type: "music".to_owned(),
    };
    assert!(lidarr(&fake).register_root_folder(&folder).await.is_ok());

    let body = fake
        .requests()
        .into_iter()
        .find(|request| request.url.contains("/rootfolder") && request.body.is_some())
        .and_then(|request| request.body)
        .unwrap_or_default();
    for expected in [
        "/data/media/music",
        "\"name\":\"music\"",
        "\"defaultQualityProfileId\":4",
        "\"defaultMetadataProfileId\":7",
    ] {
        assert!(body.contains(expected), "{expected} is missing from {body}");
    }
}

/// A service with no metadata profiles is given a path and nothing else.
///
/// Sending the music fields to one that does not file that way is not a field it
/// ignores — the extra profile ids name nothing it holds. Which service wants them is
/// asked rather than inferred from its name, so a service answering with no profiles
/// is one that neither has them nor wants them.
#[tokio::test]
async fn a_root_folder_for_a_service_without_metadata_profiles_carries_only_its_path() {
    let fake = Fake::by_path(vec![
        ("/metadataprofile", Answer::reply(404, "")),
        ("/rootfolder", Answer::reply(201, "")),
    ]);
    let folder = RootFolder {
        path: "/data/media/tv".to_owned(),
        media_type: "tv".to_owned(),
    };
    assert!(sonarr(&fake).register_root_folder(&folder).await.is_ok());

    let body = fake
        .requests()
        .into_iter()
        .find(|request| request.url.contains("/rootfolder") && request.body.is_some())
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(body.contains("/data/media/tv"), "{body}");
    assert!(
        !body.contains("defaultMetadataProfileId") && !body.contains("defaultQualityProfileId"),
        "a service that files no music was sent the music fields: {body}"
    );
}
