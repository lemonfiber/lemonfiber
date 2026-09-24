//! Tracing against the services: what is read, and what cannot be.

use super::*;

/// A Jellyfin sign-in that hands back an access token, and a library that has the
/// traced item — the pair a media server answers when the item is finally available.
const SIGNED_IN: &str = r#"{"AccessToken":"token"}"#;

const HAS_ITEM: &str = r#"{"Items":[{"Name":"The Expanse"}]}"#;

const NO_ITEM: &str = r#"{"Items":[]}"#;

/// A context whose media server the trace cannot ask — no admin password is recorded,
/// so the library stage is simply left unanswered, as on the \*arr-only slices.
fn ctx(library: &'static str, history: &'static str, queue: &'static str) -> Ctx {
    ctx_with(&Fake::arr(library, history, queue))
}

/// A context that can reach its Jellyfin: the admin password is recorded under the
/// env file, so the trace's `jellyfin_reader` resolves a reading client. Tagged so
/// each test keeps its own env file rather than racing on a shared one.
fn ctx_with_jellyfin(fake: &Fake, tag: &str) -> Ctx {
    let dir = std::env::temp_dir().join(format!("lemonfiber-trace-{tag}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let mut context = ctx_with(fake);
    context.settings.env_file = Some(dir.join(".env"));
    crate::app::targets::record_secret(
        &context,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        &a_password(),
    );
    context
}

/// A Jellyfin reading client over a transport, for the library-presence reads.
fn jellyfin(fake: &Fake) -> Jellyfin {
    Jellyfin::authenticated(
        fake.transport(),
        "http://127.0.0.1:8096",
        "jellyfin",
        crate::config::JELLYFIN_ADMIN_USER,
        a_password(),
    )
}

#[tokio::test]
async fn tracing_a_matched_item_reads_its_history_and_queue() {
    // Grabbed in history, and the queue shows it downloading — the trace reads both.
    let context = ctx(
        r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
        r#"{"records":[{"eventType":"grabbed","date":"2026-01-01T00:00:00Z"}]}"#,
        r#"{"records":[{"seriesId":1,"trackedDownloadState":"downloading","trackedDownloadStatus":"ok"}]}"#,
    );
    let report = trace(&context, "expanse", None, false)
        .await
        .unwrap_or_default();
    assert!(report.matched);
    assert_eq!(report.furthest, Stage::Downloading);
}

#[tokio::test]
async fn a_matched_item_whose_reads_fail_reports_them_unavailable() {
    // The item is found, but its history and queue come back unreadable: the trace
    // reports both as unavailable rather than inferring the item stalled.
    let context = ctx(
        r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
        "not json",
        "not json",
    );
    let report = trace(&context, "expanse", None, false)
        .await
        .unwrap_or_default();
    assert!(report.matched);
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.contains("history could not be read")));
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.contains("queue could not be read")));
}

#[tokio::test]
async fn tracing_a_term_no_item_matches_is_not_monitored() {
    let context = ctx("[]", "{}", EMPTY_QUEUE);
    let report = trace(&context, "nothing here", None, false)
        .await
        .unwrap_or_default();
    assert!(!report.matched);
    assert_eq!(report.furthest, Stage::NotMonitored);
}

#[tokio::test]
async fn tracing_passes_over_a_service_whose_library_cannot_be_read() {
    // A service that answers nonsense to the library read is passed over rather than
    // failing the whole trace; with every service unreadable, nothing matched.
    let report = trace(&ctx("not json", "{}", EMPTY_QUEUE), "expanse", None, false)
        .await
        .unwrap_or_default();
    assert!(!report.matched);
    assert_eq!(report.furthest, Stage::NotMonitored);
}

#[tokio::test]
async fn tracing_a_matched_item_present_in_the_library_reports_available() {
    // Imported in history, and the media server confirms it is in the library: the
    // trace runs all the way to available, on the Jellyfin stage, marked uncertain.
    let context = ctx_with_jellyfin(
        &Fake {
            library: r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
            history: r#"{"records":[{"eventType":"downloadFolderImported","date":"2026-01-01T00:00:00Z"}]}"#,
            queue: EMPTY_QUEUE,
            episodes: NO_EPISODES,
            sign_in: SIGNED_IN,
            jellyfin_library: HAS_ITEM,
            wanted: "",
            releases: "",
        },
        "available",
    );
    let report = trace(&context, "expanse", None, false)
        .await
        .unwrap_or_default();
    assert_eq!(report.furthest, Stage::Available);
    assert_eq!(report.confidence, crate::trace::Confidence::Uncertain);
    assert!(report
        .stages
        .iter()
        .any(|stage| stage.stage == Stage::Available && stage.service == "Jellyfin"));
}

#[tokio::test]
async fn a_media_server_with_nothing_reads_as_absent() {
    // The sign-in is accepted and the library answers, holding nothing: a confirmed
    // absence, not an unknown.
    let presence = library_presence(
        Some(&jellyfin(&Fake {
            library: "",
            history: "",
            queue: "",
            episodes: NO_EPISODES,
            sign_in: SIGNED_IN,
            jellyfin_library: NO_ITEM,
            wanted: "",
            releases: "",
        })),
        Kind::Sonarr,
        "The Expanse",
    )
    .await;
    assert_eq!(presence, Some(Presence::Absent));
}

#[tokio::test]
async fn a_media_server_that_will_not_answer_leaves_presence_unknown() {
    // The sign-in comes back as something that is not a session: the read failed, so
    // presence is unknown — never inferred as absent.
    let presence = library_presence(
        Some(&jellyfin(&Fake::arr("", "", ""))),
        Kind::Radarr,
        "The Expanse",
    )
    .await;
    assert_eq!(presence, None);
}

#[tokio::test]
async fn tracing_over_an_unreadable_stack_is_an_error() {
    let bad = a_context().over(nowhere()).build();
    assert!(trace(&bad, "anything", None, false).await.is_err());
}

/// A queue holding one stuck download, the show embedded so it can be named.
const STUCK_QUEUE: &str = r#"{"records":[{"trackedDownloadStatus":"warning","trackedDownloadState":"downloading","series":{"title":"The Expanse"}}]}"#;

#[tokio::test]
async fn stuck_lists_each_stuck_item_tagged_with_its_service() {
    // Sonarr's queue holds a stuck series; Radarr's holds nothing it can name (a series
    // record, no movie title) and Lidarr is not a traceable kind — so the one stuck
    // item is listed, tagged with the service holding it, and the list is complete.
    let report = super::super::stuck(&ctx("", "", STUCK_QUEUE))
        .await
        .unwrap_or_default();
    assert!(report
        .items
        .iter()
        .any(|entry| entry.title == "The Expanse" && entry.service == "Sonarr"));
    assert!(!report.incomplete);
}

#[tokio::test]
async fn stuck_marks_the_list_incomplete_where_a_queue_cannot_be_read() {
    // An \*arr whose queue will not decode is reported as leaving the list possibly
    // short, rather than read as nothing stuck.
    let report = super::super::stuck(&ctx("", "", "not json"))
        .await
        .unwrap_or_default();
    assert!(report.items.is_empty());
    assert!(report.incomplete);
}

#[tokio::test]
async fn stuck_over_arrs_that_have_not_started_finds_nothing() {
    // No key is readable, so no \*arr opens: nothing was asked, so the list is empty
    // and complete rather than incomplete.
    let context = a_context()
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)))
        .with_http(Fake::arr("", "", EMPTY_QUEUE).transport());
    let report = super::super::stuck(&context).await.unwrap_or_default();
    assert!(report.items.is_empty());
    assert!(!report.incomplete);
}

#[tokio::test]
async fn a_stuck_query_over_an_unreadable_stack_is_an_error() {
    let bad = a_context().over(nowhere()).build();
    assert!(super::super::stuck(&bad).await.is_err());
}
