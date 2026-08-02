//! Following one item across the services, to answer "where is my show?".
//!
//! The question a household actually asks is answerable only by correlating fragments
//! four services each hold. This is the first cut: the \*arr that monitors an item and
//! records its history is the spine, and this reads it into one view — how far the item
//! got, and, where the \*arr's own record plainly shows it stopped, why.
//!
//! What the \*arr cannot see alone — whether an indexer found releases, whether the
//! download client took it, whether the media server can play it — is left to the
//! services that can, in a later slice; this never over-claims the reason for a stall it
//! cannot prove.

use super::targets::{project_directory, servarr_targets};
use super::Ctx;
use crate::error::{Diagnose, Problem};
use crate::model::{TraceReport, TraceStage};
use crate::ports::service::{Pipeline, TraceEvent};
use crate::recyclarr::Kind;
use crate::trace::{Confidence, Stage};

/// Trace one item by a human term, across the resolution services.
///
/// Searches each \*arr's library for a title matching the term; the first match is
/// followed. No match at all is itself the answer — nobody asked for it, the first stage
/// the trace tells apart.
pub(super) async fn trace(ctx: &Ctx, term: &str) -> Result<TraceReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());

    for target in servarr_targets(&manifest.services, project.as_deref()) {
        let Some(kind) = Kind::for_section(&target.id) else {
            continue;
        };
        let Some(service) = target.open(&ctx.http, ctx.filesystem.as_ref()).await else {
            continue;
        };
        let Ok(items) = service.find_items(kind, term).await else {
            continue;
        };
        let Some(item) = items.into_iter().next() else {
            continue;
        };
        let events = service
            .item_history(kind, item.id)
            .await
            .unwrap_or_default();
        return Ok(assemble(&target.name, &item.title, item.monitored, events));
    }

    Ok(not_matched(term))
}

/// Build the trace from what one \*arr knows: the stages its history records, the
/// furthest reached, and — only where the \*arr's own record proves it — why it stopped.
fn assemble(service: &str, title: &str, monitored: bool, events: Vec<TraceEvent>) -> TraceReport {
    let reached: Vec<Stage> = events.iter().map(|event| event.stage).collect();
    let furthest = Stage::furthest(monitored, &reached);

    let mut stages = Vec::new();
    if monitored {
        stages.push(TraceStage {
            stage: Stage::Monitored,
            service: service.to_owned(),
            at: None,
        });
    }
    // The reader gives history newest-first; a trace reads oldest-first, the order things
    // happened.
    for event in events.into_iter().rev() {
        stages.push(TraceStage {
            stage: event.stage,
            service: service.to_owned(),
            at: Some(event.at),
        });
    }

    // Only the stalls the *arr's own record proves: nothing monitoring it, or monitored
    // with no history at all (nothing found). Whether a grab is stuck or still
    // downloading, or an import is playable, needs the other services — not claimed here.
    let stall = match furthest {
        Stage::NotMonitored | Stage::Monitored => furthest.stall().map(str::to_owned),
        _ => None,
    };

    TraceReport {
        item: title.to_owned(),
        matched: monitored,
        furthest,
        stall,
        stages,
        confidence: Confidence::Certain,
    }
}

/// The trace for a term no monitored item matches — nobody has asked for it.
fn not_matched(term: &str) -> TraceReport {
    TraceReport {
        item: term.to_owned(),
        matched: false,
        furthest: Stage::NotMonitored,
        stall: Stage::NotMonitored.stall().map(str::to_owned),
        stages: Vec::new(),
        confidence: Confidence::Certain,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::{assemble, trace, Ctx};
    use crate::config::Settings;
    use crate::platform::Environment;
    use crate::ports::http::{Http, Request, Response, Unreachable};
    use crate::ports::service::TraceEvent;
    use crate::stack::Source;
    use crate::test_support::{spoke, stack, Reporting, Scripted, SeedFs};
    use crate::trace::Stage;

    /// A Servarr config that opens a target, carrying a readable key.
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

    /// A transport that answers the library and history reads by the shape of the URL,
    /// so a trace's two calls need no exact ordering.
    struct Fake {
        library: &'static str,
        history: &'static str,
    }

    #[async_trait]
    impl Http for Fake {
        async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
            let body = if request.url.contains("/history") {
                self.history
            } else {
                self.library
            };
            Ok(Response {
                status: 200,
                body: body.to_owned(),
            })
        }
    }

    fn event(stage: Stage) -> TraceEvent {
        TraceEvent {
            stage,
            at: "2026-01-01T00:00:00Z".to_owned(),
        }
    }

    /// A context over the real stack, a filesystem that opens the \*arrs, and the given
    /// library/history answers.
    fn ctx(library: &'static str, history: &'static str) -> Ctx {
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            Settings::default(),
            Environment::MacOs,
        )
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .with_http(Arc::new(Fake { library, history }))
    }

    #[test]
    fn a_monitored_item_with_a_grab_and_import_reaches_imported() {
        let report = assemble(
            "Sonarr",
            "The Expanse",
            true,
            vec![event(Stage::Imported), event(Stage::Grabbed)],
        );
        assert!(report.matched);
        assert_eq!(report.furthest, Stage::Imported);
        // Monitored + the two events, oldest-first.
        assert_eq!(report.stages.len(), 3);
        assert_eq!(
            report.stages.first().map(|s| s.stage),
            Some(Stage::Monitored)
        );
        assert_eq!(report.stages.last().map(|s| s.stage), Some(Stage::Imported));
        // Reaching import is not a stall the *arr can call stuck on its own.
        assert!(report.stall.is_none());
    }

    #[test]
    fn a_monitored_item_with_no_history_stalls_as_never_found() {
        let report = assemble("Sonarr", "The Expanse", true, Vec::new());
        assert_eq!(report.furthest, Stage::Monitored);
        assert!(report
            .stall
            .as_deref()
            .is_some_and(|reason| reason.contains("indexers returned nothing")));
    }

    #[test]
    fn an_unmonitored_item_is_reported_as_nobody_asked() {
        let report = assemble("Sonarr", "The Expanse", false, Vec::new());
        assert!(!report.matched);
        assert_eq!(report.furthest, Stage::NotMonitored);
        assert!(report.stall.is_some());
    }

    #[tokio::test]
    async fn tracing_a_matched_item_reads_its_history() {
        let context = ctx(
            r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
            r#"{"records":[{"eventType":"grabbed","date":"2026-01-01T00:00:00Z"}]}"#,
        );
        let report = trace(&context, "expanse").await.unwrap_or_default();
        assert!(report.matched);
        assert_eq!(report.furthest, Stage::Grabbed);
    }

    #[tokio::test]
    async fn tracing_a_term_no_item_matches_is_not_monitored() {
        let context = ctx("[]", "{}");
        let report = trace(&context, "nothing here").await.unwrap_or_default();
        assert!(!report.matched);
        assert_eq!(report.furthest, Stage::NotMonitored);
    }

    #[tokio::test]
    async fn tracing_passes_over_a_service_whose_library_cannot_be_read() {
        // A service that answers nonsense to the library read is passed over rather than
        // failing the whole trace; with every service unreadable, nothing matched.
        let report = trace(&ctx("not json", "{}"), "expanse")
            .await
            .unwrap_or_default();
        assert!(!report.matched);
        assert_eq!(report.furthest, Stage::NotMonitored);
    }

    #[tokio::test]
    async fn tracing_over_an_unreadable_stack_is_an_error() {
        let bad = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            Source::External(std::path::Path::new("/lemonfiber/no/such/stack")),
            Settings::default(),
            Environment::MacOs,
        );
        assert!(trace(&bad, "anything").await.is_err());
    }
}
