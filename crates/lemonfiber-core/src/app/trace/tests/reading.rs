//! Tracing against the services: what is read, and what cannot be.

use super::*;

/// A context whose media server the trace cannot ask — it has not moved into its plugin,
/// so the library stage is simply left unanswered, as on the curator-only slices.
fn ctx(items: Answer, history: Answer, queue: Answer) -> Ctx {
    ctx_with(&Fake::curator(items, history, queue))
}

/// A context that can reach its media server as well as its curators, each moved into its
/// plugin.
fn ctx_with_media_server(fake: &Fake) -> Ctx {
    let mut context = ctx_with(fake);
    context.seams.engine = Arc::new(
        curators::running(&curators::ALL).alongside(crate::test_support::media::running()),
    );
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
        monitored(),
        happened(Outcome::Grabbed),
        answered(&queued(Stage::Downloading, false)),
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
    let context = ctx(monitored(), refusing(), refusing());
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
    let context = ctx(
        answered(&Vec::<FoundItem>::new()),
        nothing_happened(),
        nothing_queued(),
    );
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
    let unreadable = ctx(refusing(), nothing_happened(), nothing_queued());
    let report = trace(&unreadable, "expanse", None, false)
        .await
        .unwrap_or_default();
    assert!(!report.matched);
    assert_eq!(report.furthest, Stage::NotMonitored);
}

#[tokio::test]
async fn tracing_a_matched_item_present_in_the_library_reports_available() {
    // Imported in history, and the media server confirms it is in the library: the
    // trace runs all the way to available, on the media server stage, marked uncertain.
    let context = ctx_with_media_server(&Fake {
        holds: answered(&true),
        ..Fake::curator(monitored(), happened(Outcome::Imported), nothing_queued())
    });
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

/// The curators' reads with the television curator answering what is stuck as `stuck`.
fn stuck_as(stuck: Answer) -> Fake {
    Fake {
        stuck,
        ..Fake::curator(monitored(), nothing_happened(), nothing_queued())
    }
}

/// The television curator holds a stuck series and the others nothing, so the one stuck
/// item is listed, tagged with the service holding it, and the list is complete.
#[tokio::test]
async fn stuck_lists_each_stuck_item_tagged_with_its_service() {
    let report = super::super::stuck(&ctx_with(&stuck_as(answered(&[StuckItem {
        title: "The Expanse".to_owned(),
        stage: Stage::Downloading,
    }]))))
    .await
    .unwrap_or_default();
    assert!(report
        .items
        .iter()
        .any(|entry| entry.title == "The Expanse" && entry.service == "Sonarr"));
    assert!(!report.incomplete);
}

/// A curator that will not say what is stuck is reported as leaving the list possibly
/// short, rather than read as nothing stuck.
#[tokio::test]
async fn stuck_marks_the_list_incomplete_where_a_queue_cannot_be_read() {
    let report = super::super::stuck(&ctx_with(&stuck_as(refusing())))
        .await
        .unwrap_or_default();
    assert!(report.items.is_empty());
    assert!(report.incomplete);
}

/// No curator's adapter runs, so none can be asked: nothing was asked, so the list is
/// empty and complete rather than incomplete.
#[tokio::test]
async fn stuck_over_curators_that_have_not_started_finds_nothing() {
    let context = unmoved(&stuck_as(refusing()));
    let report = super::super::stuck(&context).await.unwrap_or_default();
    assert!(report.items.is_empty());
    assert!(!report.incomplete);
}

#[tokio::test]
async fn a_stuck_query_over_an_unreadable_stack_is_an_error() {
    let bad = a_context().over(nowhere()).build();
    assert!(super::super::stuck(&bad).await.is_err());
}
