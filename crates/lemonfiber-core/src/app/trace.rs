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

use super::targets::{jellyfin_reader, project_directory, servarr_targets};
use super::Ctx;
use crate::error::{Diagnose, Problem};
use crate::model::{TraceReport, TraceStage};
use crate::ports::service::{Library, Pipeline, QueueItem, TraceEvent};
use crate::recyclarr::Kind;
use crate::trace::{Confidence, Presence, Stage};

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
    // The media server is resolved once, ahead of the match: the last stage of a trace is
    // the same read whichever \*arr the item turns up in.
    let jellyfin = jellyfin_reader(ctx, &manifest.services);

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
        // The queue is read best-effort: it enriches the trace with what is downloading
        // now, but a trace still stands on history where the queue cannot be read.
        let queue = service.item_queue(kind, item.id).await.unwrap_or_default();
        let library = library_presence(jellyfin.as_ref(), kind, &item.title).await;
        return Ok(assemble(
            &target.name,
            &item.title,
            item.monitored,
            events,
            queue,
            library,
        ));
    }

    Ok(not_matched(term))
}

/// Ask the media server whether the item is in the library, as the three-way answer a
/// trace folds in: present, provably absent, or — where there is no media server to ask,
/// or it will not answer — unknown, which the trace reads as "cannot tell" rather than
/// inferring an availability it has not confirmed.
async fn library_presence(
    jellyfin: Option<&crate::jellyfin::Jellyfin>,
    kind: Kind,
    title: &str,
) -> Option<Presence> {
    match jellyfin?.has_item(kind, title).await {
        Ok(true) => Some(Presence::Present),
        Ok(false) => Some(Presence::Absent),
        Err(_) => None,
    }
}

/// Build the trace from what one \*arr knows and what the media server confirms: the
/// stages its history records, what its queue is doing now, whether it is finally in the
/// library, the furthest reached, and — where a record proves it — why it stopped.
fn assemble(
    service: &str,
    title: &str,
    monitored: bool,
    events: Vec<TraceEvent>,
    queue: Option<QueueItem>,
    library: Option<Presence>,
) -> TraceReport {
    // Presence in the media server only means something once an \*arr is monitoring the
    // item; for one nobody asked for, "not monitored" is the whole answer, and a stray
    // library match is a service disagreement left to a later slice, not availability.
    let library = monitored.then_some(library).flatten();

    let max_history = events.iter().map(|event| event.stage).max();
    let mut reached: Vec<Stage> = events.iter().map(|event| event.stage).collect();
    if let Some(item) = queue {
        reached.push(item.stage);
    }
    // The library is the last word on how far an item got: confirmed present, it is
    // available whatever the \*arr's own record stops at.
    let present = library == Some(Presence::Present);
    if present {
        reached.push(Stage::Available);
    }
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
    // The queue is the live state; add it only where it carries the item past what its
    // history already shows, so it is the current step rather than a repeat.
    if let Some(item) = queue {
        if max_history.is_none_or(|reached| item.stage > reached) {
            stages.push(TraceStage {
                stage: item.stage,
                service: service.to_owned(),
                at: None,
            });
        }
    }
    // The media server's confirmation is the final stage — a present fact, so untimed, and
    // always past what a \*arr's history and queue can show.
    if present {
        stages.push(TraceStage {
            stage: Stage::Available,
            service: "Jellyfin".to_owned(),
            at: None,
        });
    }

    let stuck = queue.is_some_and(|item| item.stuck);
    let stall = if stuck {
        // The C7 signal: queued but not progressing — a real problem the operator can act
        // on, distinct from a download merely still running.
        Some(
            "the download is in the queue but not progressing — the download client needs \
             attention"
                .to_owned(),
        )
    } else {
        // A grab that never made the queue and never imported is now provably stuck at
        // grabbed; the not-monitored and never-found stalls stand as before. Imported but
        // confirmed absent from the library is now provably still awaiting a scan — a
        // reason only the media server can supply, so it stands only on a confirmed
        // absence, never on a media server that could not be asked.
        match furthest {
            Stage::NotMonitored | Stage::Monitored | Stage::Grabbed => {
                furthest.stall().map(str::to_owned)
            }
            Stage::Imported if library == Some(Presence::Absent) => {
                Stage::Imported.stall().map(str::to_owned)
            }
            _ => None,
        }
    };

    TraceReport {
        item: title.to_owned(),
        matched: monitored,
        furthest,
        stall,
        stages,
        // A presence found by matching titles across to the media server — the two ends
        // share no id — may not be the item asked for, so it is marked, never claimed.
        confidence: if present {
            Confidence::Uncertain
        } else {
            Confidence::Certain
        },
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

    use super::{assemble, library_presence, trace, Ctx};
    use crate::config::Settings;
    use crate::jellyfin::Jellyfin;
    use crate::platform::Environment;
    use crate::ports::http::{Http, Request, Response, Unreachable};
    use crate::ports::service::{QueueItem, TraceEvent};
    use crate::recyclarr::Kind;
    use crate::stack::Source;
    use crate::test_support::{a_password, spoke, stack, Reporting, Scripted, SeedFs};
    use crate::trace::{Presence, Stage};

    /// A Servarr config that opens a target, carrying a readable key.
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

    /// An empty queue, as the shape a service returns with nothing downloading.
    const EMPTY_QUEUE: &str = r#"{"records":[]}"#;

    /// A Jellyfin sign-in that hands back an access token, and a library that has the
    /// traced item — the pair a media server answers when the item is finally available.
    const SIGNED_IN: &str = r#"{"AccessToken":"token"}"#;
    const HAS_ITEM: &str = r#"{"Items":[{"Name":"The Expanse"}]}"#;
    const NO_ITEM: &str = r#"{"Items":[]}"#;

    /// A transport that answers each service's reads by the shape of the URL, so a trace's
    /// calls need no exact ordering: the \*arr library, history and queue, and Jellyfin's
    /// sign-in and library.
    struct Fake {
        library: &'static str,
        history: &'static str,
        queue: &'static str,
        sign_in: &'static str,
        jellyfin_library: &'static str,
    }

    impl Fake {
        /// A transport with no media server configured to answer — the \*arr-only reads
        /// the trace made before it could see the library.
        fn arr(library: &'static str, history: &'static str, queue: &'static str) -> Self {
            Self {
                library,
                history,
                queue,
                sign_in: "",
                jellyfin_library: "",
            }
        }
    }

    #[async_trait]
    impl Http for Fake {
        async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
            let body = if request.url.contains("/AuthenticateByName") {
                self.sign_in
            } else if request.url.contains("/Items") {
                self.jellyfin_library
            } else if request.url.contains("/history") {
                self.history
            } else if request.url.contains("/queue") {
                self.queue
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

    /// A context over the real stack, a filesystem that opens the \*arrs, and a transport
    /// answering the given reads.
    fn ctx_with(fake: Fake) -> Ctx {
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
        .with_http(Arc::new(fake))
    }

    /// A context whose media server the trace cannot ask — no admin password is recorded,
    /// so the library stage is simply left unanswered, as on the \*arr-only slices.
    fn ctx(library: &'static str, history: &'static str, queue: &'static str) -> Ctx {
        ctx_with(Fake::arr(library, history, queue))
    }

    /// A context that can reach its Jellyfin: the admin password is recorded under the
    /// env file, so the trace's `jellyfin_reader` resolves a reading client. Tagged so
    /// each test keeps its own env file rather than racing on a shared one.
    fn ctx_with_jellyfin(fake: Fake, tag: &str) -> Ctx {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-trace-{tag}-{}", std::process::id()));
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
    fn jellyfin(fake: Fake) -> Jellyfin {
        Jellyfin::authenticated(
            Arc::new(fake),
            "http://127.0.0.1:8096",
            "jellyfin",
            crate::config::JELLYFIN_ADMIN_USER,
            a_password(),
        )
    }

    #[test]
    fn a_monitored_item_with_a_grab_and_import_reaches_imported() {
        let report = assemble(
            "Sonarr",
            "The Expanse",
            true,
            vec![event(Stage::Imported), event(Stage::Grabbed)],
            None,
            None,
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
        // Imported with the media server unread is not a stall: it may already be
        // scanned, and the *arr cannot tell — so nothing is claimed.
        assert!(report.stall.is_none());
    }

    #[test]
    fn a_monitored_item_with_no_history_stalls_as_never_found() {
        let report = assemble("Sonarr", "The Expanse", true, Vec::new(), None, None);
        assert_eq!(report.furthest, Stage::Monitored);
        assert!(report
            .stall
            .as_deref()
            .is_some_and(|reason| reason.contains("indexers returned nothing")));
    }

    #[test]
    fn an_unmonitored_item_is_reported_as_nobody_asked() {
        let report = assemble("Sonarr", "The Expanse", false, Vec::new(), None, None);
        assert!(!report.matched);
        assert_eq!(report.furthest, Stage::NotMonitored);
        assert!(report.stall.is_some());
    }

    #[test]
    fn a_grab_that_never_reached_the_queue_is_stuck_at_grabbed() {
        // Grabbed in history, nothing in the queue, never imported: the download client
        // never took it — now provable, so it stalls at grabbed.
        let report = assemble(
            "Sonarr",
            "The Expanse",
            true,
            vec![event(Stage::Grabbed)],
            None,
            None,
        );
        assert_eq!(report.furthest, Stage::Grabbed);
        assert!(report
            .stall
            .as_deref()
            .is_some_and(|reason| reason.contains("download client never took it")));
    }

    #[test]
    fn a_queued_download_carries_the_trace_to_downloading() {
        // Grabbed in history and downloading in the queue: the queue advances the trace,
        // and downloading is in progress — not a stall.
        let report = assemble(
            "Sonarr",
            "The Expanse",
            true,
            vec![event(Stage::Grabbed)],
            Some(QueueItem {
                stage: Stage::Downloading,
                stuck: false,
            }),
            None,
        );
        assert_eq!(report.furthest, Stage::Downloading);
        assert_eq!(
            report.stages.last().map(|s| s.stage),
            Some(Stage::Downloading)
        );
        assert!(report.stall.is_none());
    }

    #[test]
    fn a_stuck_queue_item_stalls_whatever_stage_it_reached() {
        let report = assemble(
            "Sonarr",
            "The Expanse",
            true,
            vec![event(Stage::Grabbed)],
            Some(QueueItem {
                stage: Stage::Downloading,
                stuck: true,
            }),
            None,
        );
        assert!(report
            .stall
            .as_deref()
            .is_some_and(|reason| reason.contains("not progressing")));
    }

    #[test]
    fn a_monitored_item_present_in_the_library_reaches_available() {
        // The media server confirms it: the trace runs to available, ends on the library
        // stage, and — a cross-service title match — is marked uncertain, not claimed.
        let report = assemble(
            "Sonarr",
            "The Expanse",
            true,
            vec![event(Stage::Imported)],
            None,
            Some(Presence::Present),
        );
        assert_eq!(report.furthest, Stage::Available);
        assert_eq!(
            report.stages.last().map(|s| s.stage),
            Some(Stage::Available)
        );
        assert_eq!(report.confidence, crate::trace::Confidence::Uncertain);
        assert!(report.stall.is_none());
    }

    #[test]
    fn imported_but_absent_from_the_library_stalls_as_not_scanned() {
        // Imported to disk, and the media server confirms it is not there: now provably
        // still waiting for the library to be scanned — a stall only a confirmed absence
        // earns, and the confidence stays certain because no fuzzy match was made.
        let report = assemble(
            "Sonarr",
            "The Expanse",
            true,
            vec![event(Stage::Imported)],
            None,
            Some(Presence::Absent),
        );
        assert_eq!(report.furthest, Stage::Imported);
        assert_eq!(report.confidence, crate::trace::Confidence::Certain);
        assert!(report
            .stall
            .as_deref()
            .is_some_and(|reason| reason.contains("has not been scanned")));
    }

    #[test]
    fn a_library_match_for_an_unmonitored_item_is_not_availability() {
        // Nobody asked for it, yet the library has something by that title: that is a
        // service disagreement, left to a later slice — "not monitored" is still the
        // whole answer, and the stray match never promotes it to available.
        let report = assemble(
            "Sonarr",
            "The Expanse",
            false,
            Vec::new(),
            None,
            Some(Presence::Present),
        );
        assert_eq!(report.furthest, Stage::NotMonitored);
        assert_eq!(report.confidence, crate::trace::Confidence::Certain);
        assert!(report
            .stall
            .as_deref()
            .is_some_and(|reason| reason.contains("nobody has asked")));
    }

    #[tokio::test]
    async fn tracing_a_matched_item_reads_its_history_and_queue() {
        // Grabbed in history, and the queue shows it downloading — the trace reads both.
        let context = ctx(
            r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
            r#"{"records":[{"eventType":"grabbed","date":"2026-01-01T00:00:00Z"}]}"#,
            r#"{"records":[{"seriesId":1,"trackedDownloadState":"downloading","trackedDownloadStatus":"ok"}]}"#,
        );
        let report = trace(&context, "expanse").await.unwrap_or_default();
        assert!(report.matched);
        assert_eq!(report.furthest, Stage::Downloading);
    }

    #[tokio::test]
    async fn tracing_a_term_no_item_matches_is_not_monitored() {
        let context = ctx("[]", "{}", EMPTY_QUEUE);
        let report = trace(&context, "nothing here").await.unwrap_or_default();
        assert!(!report.matched);
        assert_eq!(report.furthest, Stage::NotMonitored);
    }

    #[tokio::test]
    async fn tracing_passes_over_a_service_whose_library_cannot_be_read() {
        // A service that answers nonsense to the library read is passed over rather than
        // failing the whole trace; with every service unreadable, nothing matched.
        let report = trace(&ctx("not json", "{}", EMPTY_QUEUE), "expanse")
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
            Fake {
                library: r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
                history: r#"{"records":[{"eventType":"downloadFolderImported","date":"2026-01-01T00:00:00Z"}]}"#,
                queue: EMPTY_QUEUE,
                sign_in: SIGNED_IN,
                jellyfin_library: HAS_ITEM,
            },
            "available",
        );
        let report = trace(&context, "expanse").await.unwrap_or_default();
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
            Some(&jellyfin(Fake {
                library: "",
                history: "",
                queue: "",
                sign_in: SIGNED_IN,
                jellyfin_library: NO_ITEM,
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
            Some(&jellyfin(Fake::arr("", "", ""))),
            Kind::Radarr,
            "The Expanse",
        )
        .await;
        assert_eq!(presence, None);
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
