//! Tracing against the services: what is read, and what cannot be.

use super::*;

/// What the media server answers asked whether its library holds the traced item.
const HOLDS: &str = "true";

/// A context whose media server the trace cannot ask — it has not moved into its plugin,
/// so the library stage is simply left unanswered, as on the curator-only slices.
fn ctx(library: &'static str, history: &'static str, queue: &'static str) -> Ctx {
    ctx_with(&Fake::curator(library, history, queue))
}

/// A context that can reach its media server: moved into its plugin, its adapter keyed
/// and published, with an env file of the test's own under `tag`.
fn ctx_with_media_server(fake: &Fake, tag: &str) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("trace-{tag}")).kept();
    let _ = std::fs::create_dir_all(&dir);
    let mut context = a_context()
        .engine(Arc::new(crate::test_support::media::running()))
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED))))
        .with_http(fake.transport());
    context.settings.env_file = Some(dir.join(".env"));
    crate::test_support::media::moved_in(&context);
    context
}

/// A library answering every question about what it holds with whether it holds it, or
/// as one that cannot be reached where nothing is said.
struct Holding(Option<bool>);

#[async_trait::async_trait]
impl crate::ports::service::Library for Holding {
    async fn has_item(
        &self,
        _kind: Kind,
        _term: &str,
    ) -> Result<bool, crate::ports::service::Failure> {
        self.0
            .ok_or_else(|| crate::ports::service::Failure::Unavailable {
                service: "jellyfin".to_owned(),
            })
    }

    async fn rescan(&self) -> Result<(), crate::ports::service::Failure> {
        Ok(())
    }
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
    // trace runs all the way to available, on the media server stage, marked uncertain.
    let context = ctx_with_media_server(
        &Fake {
            library: r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
            history: r#"{"records":[{"eventType":"downloadFolderImported","date":"2026-01-01T00:00:00Z"}]}"#,
            queue: EMPTY_QUEUE,
            episodes: NO_EPISODES,
            holds: HOLDS,
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
    // The library answers, holding nothing: a confirmed absence, not an unknown.
    let presence = library_presence(Some(&Holding(Some(false))), Kind::Tv, "The Expanse").await;
    assert_eq!(presence, Some(Presence::Absent));
}

#[tokio::test]
async fn a_media_server_that_will_not_answer_leaves_presence_unknown() {
    // The library cannot be read: presence is unknown — never inferred as absent.
    let presence = library_presence(Some(&Holding(None)), Kind::Movies, "The Expanse").await;
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
    // The TV curator's queue holds a stuck series; the movie curator's holds nothing it can
    // name (a series record, no movie title) and the music curator is not a traceable kind
    // — so the one stuck item is listed, tagged with the service holding it, and the list
    // is complete.
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
    // A curator whose queue will not decode is reported as leaving the list possibly
    // short, rather than read as nothing stuck.
    let report = super::super::stuck(&ctx("", "", "not json"))
        .await
        .unwrap_or_default();
    assert!(report.items.is_empty());
    assert!(report.incomplete);
}

#[tokio::test]
async fn stuck_over_curators_that_have_not_started_finds_nothing() {
    // No key is readable, so no curator opens: nothing was asked, so the list is empty
    // and complete rather than incomplete.
    let context = a_context()
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(None)))
        .with_http(Fake::curator("", "", EMPTY_QUEUE).transport());
    let report = super::super::stuck(&context).await.unwrap_or_default();
    assert!(report.items.is_empty());
    assert!(!report.incomplete);
}

#[tokio::test]
async fn a_stuck_query_over_an_unreadable_stack_is_an_error() {
    let bad = a_context().over(nowhere()).build();
    assert!(super::super::stuck(&bad).await.is_err());
}
