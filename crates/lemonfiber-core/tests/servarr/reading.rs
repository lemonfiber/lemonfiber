//! What is read back from a service: folders, clients, keys and queues.

use crate::{lidarr, sabnzbd, sonarr, taking_on};
use lemonfiber_core::ports::http::Http;
use lemonfiber_core::ports::service::{
    Category, Failure, QueueDepth, Queued, RegisteredClient, RegisteredFolder, RootFolder,
};
use lemonfiber_core::recyclarr::Kind;
use lemonfiber_core::servarr::{api_key, Servarr};
use lemonfiber_fixtures::http::{Answer, Fake};
use std::sync::Arc;

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

/// Each \*arr is asked in its own vocabulary, and never in the other's.
///
/// The two services take the same request under different names — a different external
/// identifier, a different search option, and one extra field each that the other has
/// never heard of. A field a service does not know is not a field it ignores: it refuses
/// the whole body, so a single shape sent to both takes nothing on anywhere.
#[tokio::test]
async fn each_kind_is_asked_in_the_vocabulary_its_own_service_answers_to() {
    let television = taking_on(Kind::Sonarr)
        .await
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(television.contains(r#""tvdbId":45745"#), "{television}");
    assert!(
        television.contains("searchForMissingEpisodes") && television.contains("seasonFolder"),
        "{television}"
    );
    assert!(
        !television.contains("tmdbId") && !television.contains("minimumAvailability"),
        "television was asked in the film service's words: {television}"
    );

    let film = taking_on(Kind::Radarr)
        .await
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(film.contains(r#""tmdbId":45745"#), "{film}");
    assert!(
        film.contains("searchForMovie") && film.contains(r#""minimumAvailability":"released""#),
        "{film}"
    );
    assert!(
        !film.contains("tvdbId") && !film.contains("seasonFolder"),
        "a film was asked in television's words: {film}"
    );
}

/// And it is put to the library each kind keeps its own things in.
#[tokio::test]
async fn each_kind_is_asked_at_its_own_library() {
    assert!(
        taking_on(Kind::Sonarr)
            .await
            .is_some_and(|request| request.url.ends_with("/series")),
        "television was not put to the series library"
    );
    assert!(
        taking_on(Kind::Radarr)
            .await
            .is_some_and(|request| request.url.ends_with("/movie")),
        "a film was not put to the film library"
    );
}
