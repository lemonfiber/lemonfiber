//! One item's journey through a \*arr: its library, its history and its queue.
//!
//! The reads a trace needs, kept apart from the writes that wire the stack —
//! the same split the adapter itself makes.
use std::sync::Arc;

use lemonfiber_core::ports::http::{Http, Method};
use lemonfiber_core::ports::service::{Importing, Pipeline, QueueItem, Queues};
use lemonfiber_core::recyclarr::Kind;
use lemonfiber_core::servarr::Servarr;
use lemonfiber_core::trace::{Outcome, Stage};
use lemonfiber_fixtures::http::{Answer, Fake};

/// A Sonarr client over the given fake — the v3 the media *arrs answer at.
fn sonarr(fake: &Arc<Fake>) -> Servarr {
    let http: Arc<dyn Http> = fake.clone();
    Servarr::new(http, "http://sonarr:8989", "the-key", "sonarr", 3)
}

// ---- Pipeline (item trace fragment) ----

/// A Sonarr client (v3) over the given router.
fn sonarr_routed(router: &Arc<Fake>) -> Servarr {
    let http: Arc<dyn Http> = router.clone();
    Servarr::new(http, "http://sonarr:8989", "the-key", "sonarr", 3)
}

#[tokio::test]
async fn find_items_matches_the_library_by_human_title() {
    let router = Fake::by_route(vec![(
        Method::Get,
        "/series",
        Answer::reply(
            200,
            r#"[{"id":1,"title":"The Expanse","monitored":true},
            {"id":2,"title":"Foundation","monitored":false}]"#
                .to_owned(),
        ),
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
    let router = Fake::by_route(vec![(
        Method::Get,
        "/movie",
        Answer::reply(
            200,
            r#"[{"id":7,"title":"Dune","monitored":true}]"#.to_owned(),
        ),
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
        .requests()
        .iter()
        .any(|request| request.url.ends_with("/api/v3/movie")));
}

#[tokio::test]
async fn item_history_keeps_the_notable_events_and_drops_the_rest() {
    let router = Fake::by_route(vec![(
        Method::Get,
        "/history",
        Answer::reply(
            200,
            r#"{"records":[
            {"eventType":"downloadFolderImported","date":"2026-01-02T00:00:00Z","episodeId":42},
            {"eventType":"downloadFailed","date":"2026-01-01T12:00:00Z"},
            {"eventType":"grabbed","date":"2026-01-01T00:00:00Z","episodeId":42},
            {"eventType":"episodeFileRenamed","date":"2025-12-31T00:00:00Z"}
        ]}"#
            .to_owned(),
        ),
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
        .requests()
        .iter()
        .any(|request| request.url.contains("seriesIds=1")));
}

#[tokio::test]
async fn an_unreadable_library_is_a_failure() {
    let router = Fake::by_route(vec![(
        Method::Get,
        "/series",
        Answer::reply(200, "not json".to_owned()),
    )]);
    assert!(sonarr_routed(&router)
        .find_items(Kind::Sonarr, "x")
        .await
        .is_err());
}

#[tokio::test]
async fn an_unreadable_history_is_a_failure() {
    let router = Fake::by_route(vec![(
        Method::Get,
        "/history",
        Answer::reply(200, "not json".to_owned()),
    )]);
    assert!(sonarr_routed(&router)
        .item_history(Kind::Sonarr, 1)
        .await
        .is_err());
}

#[tokio::test]
async fn item_queue_reads_a_downloading_item_by_series() {
    let router = Fake::by_route(vec![(Method::Get, "/queue", Answer::reply(200, r#"{"records":[
            {"seriesId":1,"episodeId":42,"trackedDownloadState":"downloading","trackedDownloadStatus":"ok"}
        ]}"#
        .to_owned()))]);
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
    let router = Fake::by_route(vec![(
        Method::Get,
        "/queue",
        Answer::reply(
            200,
            r#"{"records":[
            {"movieId":7,"trackedDownloadState":"stalled","trackedDownloadStatus":"warning"}
        ]}"#
            .to_owned(),
        ),
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
    let router = Fake::by_route(vec![
        (Method::Get, "/queue?page=1", Answer::reply(200, page_one)),
        (Method::Get, "/queue?page=2", Answer::reply(200, page_two)),
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
        .requests()
        .iter()
        .any(|request| request.url.contains("page=2")));
}

#[tokio::test]
async fn item_queue_holding_nothing_for_the_item_is_empty() {
    let router = Fake::by_route(vec![(
        Method::Get,
        "/queue",
        Answer::reply(
            200,
            r#"{"records":[{"seriesId":99,"trackedDownloadState":"downloading"}]}"#.to_owned(),
        ),
    )]);
    let queue = sonarr_routed(&router)
        .item_queue(Kind::Sonarr, 1)
        .await
        .unwrap_or_default();
    assert!(queue.is_empty());
}

#[tokio::test]
async fn item_parts_reads_the_episodes_of_a_series() {
    let router = Fake::by_route(vec![(Method::Get, "/episode", Answer::reply(200, r#"[
            {"id":11,"seasonNumber":1,"episodeNumber":1,"title":"Dulcinea","monitored":true,"hasFile":true},
            {"id":12,"seasonNumber":1,"episodeNumber":2,"title":"The Big Empty","monitored":true,"hasFile":false},
            {"id":13,"seasonNumber":0,"episodeNumber":1,"title":"A Special"}
        ]"#
        .to_owned()))]);
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
        .requests()
        .iter()
        .any(|request| request.url.contains("seriesId=1")));
}

#[tokio::test]
async fn item_parts_narrows_to_one_season_at_the_service() {
    let router = Fake::by_route(vec![(
        Method::Get,
        "/episode",
        Answer::reply(200, "[]".to_owned()),
    )]);
    let parts = sonarr_routed(&router)
        .item_parts(Kind::Sonarr, 1, Some(2))
        .await
        .unwrap_or_default();
    assert!(parts.is_empty());
    // The season filter is the service's own, not a slice taken after reading them all.
    assert!(router
        .requests()
        .iter()
        .any(|request| request.url.contains("seasonNumber=2")));
}

#[tokio::test]
async fn a_film_has_no_parts_and_is_never_asked_for_them() {
    // A film is the whole item. Asking a service that files nothing per part would be a
    // request with no answer, so none is made.
    let router = Fake::by_route(Vec::new());
    let radarr = {
        let http: Arc<dyn Http> = router.clone();
        Servarr::new(http, "http://radarr:7878", "the-key", "radarr", 3)
    };
    let parts = radarr
        .item_parts(Kind::Radarr, 7, None)
        .await
        .unwrap_or_default();
    assert!(parts.is_empty());
    assert!(router.requests().is_empty());
}

#[tokio::test]
async fn unreadable_episodes_are_a_failure() {
    let router = Fake::by_route(vec![(
        Method::Get,
        "/episode",
        Answer::reply(200, "not json".to_owned()),
    )]);
    assert!(sonarr_routed(&router)
        .item_parts(Kind::Sonarr, 1, None)
        .await
        .is_err());
}

#[tokio::test]
async fn stuck_items_names_each_stuck_show_once() {
    // Five queued records: two stuck episodes of one show, a healthy one, a stuck one whose
    // embedded title is empty, and a stuck one with no show at all.
    let router = Fake::by_route(vec![(Method::Get, "/queue", Answer::reply(200, r#"{"records":[
            {"trackedDownloadStatus":"warning","trackedDownloadState":"downloading","series":{"title":"The Expanse"}},
            {"trackedDownloadStatus":"error","trackedDownloadState":"importPending","series":{"title":"The Expanse"}},
            {"trackedDownloadStatus":"ok","trackedDownloadState":"downloading","series":{"title":"Not Stuck"}},
            {"trackedDownloadStatus":"warning","trackedDownloadState":"downloading","series":{"title":""}},
            {"trackedDownloadStatus":"warning","trackedDownloadState":"downloading"}
        ]}"#
        .to_owned()))]);
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
        .requests()
        .iter()
        .any(|request| request.url.contains("includeSeries=true")));
}

#[tokio::test]
async fn stuck_items_names_a_stuck_film_by_its_movie() {
    let router = Fake::by_route(vec![(Method::Get, "/queue", Answer::reply(200, r#"{"records":[{"trackedDownloadStatus":"error","trackedDownloadState":"downloading","movie":{"title":"Dune"}}]}"#
            .to_owned()))]);
    let radarr = {
        let http: Arc<dyn Http> = router.clone();
        Servarr::new(http, "http://radarr:7878", "the-key", "radarr", 3)
    };
    let items = radarr.stuck_items(Kind::Radarr).await.unwrap_or_default();
    assert_eq!(items.first().map(|item| item.title.as_str()), Some("Dune"));
    assert!(router
        .requests()
        .iter()
        .any(|request| request.url.contains("includeMovie=true")));
}

#[tokio::test]
async fn an_unreadable_queue_is_a_failure() {
    let router = Fake::by_route(vec![(
        Method::Get,
        "/queue",
        Answer::reply(200, "not json".to_owned()),
    )]);
    assert!(sonarr_routed(&router)
        .item_queue(Kind::Sonarr, 1)
        .await
        .is_err());
}

#[tokio::test]
async fn whether_the_service_hardlinks_is_read_from_its_own_settings() {
    let fake = Fake::always(Answer::reply(200, r#"{"id":1,"copyUsingHardlinks":true}"#));
    assert_eq!(sonarr(&fake).hardlinks().await.ok(), Some(true));
}

#[tokio::test]
async fn telling_it_to_copy_keeps_every_other_setting_it_had() {
    // The service replaces the whole document on a write, so sending only the one
    // field would silently reset settings the operator chose themselves.
    let fake = Fake::always(Answer::reply(
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
    let fake = Fake::always(Answer::reply(200, "[]"));
    assert!(sonarr(&fake).set_hardlinks(false).await.is_err());
}

#[tokio::test]
async fn an_item_fetched_over_and_over_carries_the_count_the_history_shows() {
    // The queue alone says one record, downloading, nothing wrong. What makes it a
    // loop is in the history — and counted per item, because the third fetch is a
    // different release from the first.
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/queue",
            Answer::reply(
                200,
                r#"{"totalRecords":1,"records":[{"title":"Some.Release.v3","episodeId":7,
               "trackedDownloadStatus":"ok","trackedDownloadState":"downloading"}]}"#
                    .to_owned(),
            ),
        ),
        (
            Method::Get,
            "/history",
            Answer::reply(
                200,
                r#"{"records":[{"eventType":"grabbed","episodeId":7},
               {"eventType":"downloadFailed","episodeId":7},
               {"eventType":"grabbed","episodeId":7},
               {"eventType":"downloadFailed","episodeId":7},
               {"eventType":"grabbed","episodeId":7}]}"#
                    .to_owned(),
            ),
        ),
    ]);
    let read = sonarr_routed(&router)
        .queue()
        .await
        .ok()
        .unwrap_or_default();
    assert_eq!(read.items.first().map(|item| item.grabs), Some(3));
}

#[tokio::test]
async fn an_item_grabbed_again_after_it_imported_is_not_counted_as_a_loop() {
    // An upgrade: a better copy replacing one that arrived. Counting the grabs
    // before the import would flag every upgraded episode on the machine.
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/queue",
            Answer::reply(
                200,
                r#"{"totalRecords":1,"records":[{"title":"Some.Release.2160p","episodeId":7,
               "trackedDownloadStatus":"ok","trackedDownloadState":"downloading"}]}"#
                    .to_owned(),
            ),
        ),
        (
            Method::Get,
            "/history",
            Answer::reply(
                200,
                r#"{"records":[{"eventType":"grabbed","episodeId":7},
               {"eventType":"downloadFolderImported","episodeId":7},
               {"eventType":"grabbed","episodeId":7},
               {"eventType":"grabbed","episodeId":7}]}"#
                    .to_owned(),
            ),
        ),
    ]);
    let read = sonarr_routed(&router)
        .queue()
        .await
        .ok()
        .unwrap_or_default();
    assert_eq!(read.items.first().map(|item| item.grabs), Some(1));
}

#[tokio::test]
async fn a_history_that_cannot_be_read_still_answers_with_the_queue() {
    // Losing the queue because a second read failed would turn a missing count
    // into a missing queue — and a count nobody could take is not a loop.
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/queue",
            Answer::reply(
                200,
                r#"{"totalRecords":1,"records":[{"title":"Some.Release","episodeId":7,
               "trackedDownloadStatus":"ok","trackedDownloadState":"downloading"}]}"#
                    .to_owned(),
            ),
        ),
        (
            Method::Get,
            "/history",
            Answer::reply(200, "not json at all".to_owned()),
        ),
    ]);
    let read = sonarr_routed(&router)
        .queue()
        .await
        .ok()
        .unwrap_or_default();
    assert_eq!(read.items.first().map(|item| item.grabs), Some(1));
    assert_eq!(read.total, 1, "the queue itself still came back");
}

#[tokio::test]
async fn a_history_the_service_refuses_still_answers_with_the_queue() {
    // The other way the second read fails: the service answers, and refuses.
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/queue",
            Answer::reply(
                200,
                r#"{"totalRecords":1,"records":[{"title":"Some.Release","episodeId":7,
               "trackedDownloadStatus":"ok","trackedDownloadState":"downloading"}]}"#
                    .to_owned(),
            ),
        ),
        (
            Method::Get,
            "/history",
            Answer::reply(500, "nope".to_owned()),
        ),
    ]);
    let read = sonarr_routed(&router)
        .queue()
        .await
        .ok()
        .unwrap_or_default();
    assert_eq!(read.items.first().map(|item| item.grabs), Some(1));
    assert_eq!(read.total, 1, "the queue itself still came back");
}
