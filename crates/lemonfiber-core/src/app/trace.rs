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
use crate::model::{StuckEntry, StuckReport, TraceMoment, TraceReport, TraceStage};
use crate::ports::service::{ItemPart, Library, Pipeline, QueueItem, TraceEvent};
use crate::recyclarr::Kind;
use crate::trace::{Confidence, Coverage, Part, Presence, Stage};

/// Trace one item by a human term, across the resolution services.
///
/// Searches each \*arr's library for a title matching the term; the first match is
/// followed. No match at all is itself the answer — nobody asked for it, the first stage
/// the trace tells apart.
pub(super) async fn trace(
    ctx: &Ctx,
    term: &str,
    season: Option<u32>,
) -> Result<TraceReport, Box<Problem>> {
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
        // Each read that can fail is kept as read-or-not, never collapsed to empty: a
        // history or queue that could not be read is not "nothing happened", so the trace
        // must not infer a stall from a silence it never actually heard.
        let (events, history) = match service.item_history(kind, item.id).await {
            Ok(events) => (events, true),
            Err(_) => (Vec::new(), false),
        };
        let (queue, queue_read) = match service.item_queue(kind, item.id).await {
            Ok(queue) => (queue, true),
            Err(_) => (Vec::new(), false),
        };
        // An item made of parts — a series — is aggregated per part, so "the show is
        // imported" cannot stand on one episode having landed. A service whose items have
        // no parts answers with none, and the trace reads as it always did.
        let (parts, parts_read) = match service.item_parts(kind, item.id, season).await {
            Ok(parts) => (parts, true),
            Err(_) => (Vec::new(), false),
        };
        let reads = Reads {
            history,
            queue: queue_read,
            parts: parts_read,
        };
        let library = library_presence(jellyfin.as_ref(), kind, &item.title).await;
        return Ok(assemble(
            &target.name,
            &item.title,
            item.monitored,
            Fragments {
                events,
                queue,
                parts,
                library,
                reads,
            },
        ));
    }

    Ok(not_matched(term))
}

/// The items whose downloads are stuck, across the \*arrs — the landing point queue
/// health leads to, so "N items stuck" becomes a named list each entry of which traces on
/// its own. An \*arr that answered but whose queue would not read marks the list
/// incomplete rather than being read as nothing stuck — the same honesty a trace keeps
/// about a silence it did not hear. One that has not finished starting, its key not yet
/// readable, is skipped as it is everywhere else: a service still coming up holds nothing
/// stuck, so its absence understates nothing.
pub(super) async fn stuck(ctx: &Ctx) -> Result<StuckReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());

    let mut items = Vec::new();
    let mut incomplete = false;
    for target in servarr_targets(&manifest.services, project.as_deref()) {
        let Some(kind) = Kind::for_section(&target.id) else {
            continue;
        };
        let Some(service) = target.open(&ctx.http, ctx.filesystem.as_ref()).await else {
            continue;
        };
        match service.stuck_items(kind).await {
            Ok(stuck) => items.extend(stuck.into_iter().map(|item| StuckEntry {
                title: item.title,
                service: target.name.clone(),
                stage: item.stage,
            })),
            Err(_) => incomplete = true,
        }
    }
    Ok(StuckReport { items, incomplete })
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

/// What the services could tell about the item: the stage-advancing events, what the queue
/// holds now, whether the media server has it — and, for each read that can fail, whether
/// it was actually read. An unreadable fragment is not an empty one, so the trace can tell
/// "nothing happened" apart from "this could not be read".
struct Fragments {
    events: Vec<TraceEvent>,
    queue: Vec<QueueItem>,
    parts: Vec<ItemPart>,
    library: Option<Presence>,
    reads: Reads,
}

/// Which of the fragments the services actually answered with. They travel together
/// because they mean one thing — what the trace is entitled to conclude from a silence —
/// and because a run of loose booleans is the shape a caller transposes without noticing.
#[derive(Debug, Clone, Copy)]
struct Reads {
    history: bool,
    queue: bool,
    parts: bool,
}

#[cfg(test)]
impl Reads {
    /// Every fragment answered — the ordinary case.
    const ALL: Self = Self {
        history: true,
        queue: true,
        parts: true,
    };
}

/// Build the trace from what one \*arr knows and what the media server confirms: the
/// stages its history records, what its queue is doing now, whether it is finally in the
/// library, the furthest reached, and — where a record proves it — why it stopped.
fn assemble(service: &str, title: &str, monitored: bool, fragments: Fragments) -> TraceReport {
    let Fragments {
        events,
        queue,
        parts,
        library,
        reads,
    } = fragments;
    // Presence in the media server only means something for availability once an \*arr is
    // monitoring the item; for one nobody asked for, "not monitored" is the whole answer,
    // and a library match is not availability but a disagreement — surfaced below as a
    // finding, never folded into how far the item got.
    let unmanaged_but_present = !monitored && library == Some(Presence::Present);
    let library = monitored.then_some(library).flatten();

    // The queue holds one record per part, so the item as a whole is the furthest any of
    // them reached and stuck if any one of them is. The per-part detail is kept for the
    // coverage below, where it is what tells a download in flight from a grab gone quiet.
    let queue_stage = queue.iter().map(|record| record.stage).max();
    let queue_stuck = queue.iter().any(|record| record.stuck);

    // The stages the history advances the item through — a grab, an import. A failed
    // download or a removal is history to show but advances no stage, so it is left out of
    // how far the item got.
    let advancing: Vec<Stage> = events
        .iter()
        .filter_map(|event| event.outcome.stage())
        .collect();
    let max_history = advancing.iter().copied().max();
    let mut reached = advancing;
    reached.extend(queue_stage);
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
    // happened — building both the stages it advanced through and the full log of what was
    // tried, so a repeated grab or a download that failed is seen rather than flattened
    // into the single furthest stage.
    let mut history = Vec::new();
    for event in events.into_iter().rev() {
        if let Some(stage) = event.outcome.stage() {
            stages.push(TraceStage {
                stage,
                service: service.to_owned(),
                at: Some(event.at.clone()),
            });
        }
        history.push(TraceMoment {
            outcome: event.outcome,
            at: event.at,
        });
    }
    // The queue is the live state; add it only where it carries the item past what its
    // history already shows, so it is the current step rather than a repeat.
    if let Some(stage) = queue_stage {
        if max_history.is_none_or(|reached| stage > reached) {
            stages.push(TraceStage {
                stage,
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

    TraceReport {
        item: title.to_owned(),
        matched: monitored,
        furthest,
        stall: stall_reason(furthest, queue_stuck, library, reads),
        stages,
        history,
        // Only an item made of parts has coverage to report. A film is the whole item,
        // and one with no parts read is one whose parts could not be read — reported as
        // unavailable below, never as a series with nothing in it.
        coverage: (!parts.is_empty()).then(|| coverage_of(parts, &queue)),
        findings: trace_findings(unmanaged_but_present, reads),
        // A presence found by matching titles across to the media server — the two ends
        // share no id — may not be the item asked for, so it is marked, never claimed.
        confidence: if present {
            Confidence::Uncertain
        } else {
            Confidence::Certain
        },
    }
}

/// Aggregate an item's parts into per-season coverage, each part's resting stage lifted by
/// what the queue is doing with it now.
///
/// The lift is the point: a part the service records as grabbed and nothing more has, on
/// its own record, been handed to a download client that never took it — but a queue record
/// for that same part says it is downloading right now. Without the join, every episode in
/// flight would read as a stalled grab, which is the one reading a trace exists to prevent.
fn coverage_of(parts: Vec<ItemPart>, queue: &[QueueItem]) -> Coverage {
    Coverage::of(
        parts
            .into_iter()
            .map(|part| {
                let resting = Stage::of_part(part.monitored, part.has_file, part.grabbed);
                let live = queue
                    .iter()
                    .filter(|record| record.part == Some(part.id))
                    .map(|record| record.stage)
                    .max();
                Part {
                    season: part.season,
                    number: part.number,
                    title: part.title,
                    stage: live.map_or(resting, |stage| resting.max(stage)),
                }
            })
            .collect(),
    )
}

/// The disagreements and unreadable-fragment notes a trace surfaces on their own, apart
/// from the linear pipeline: a media server holding what nothing monitors, and each read
/// that failed reported as unavailable rather than inferred as nothing — the honesty the
/// trace keeps about a silence it did not actually hear.
fn trace_findings(unmanaged_but_present: bool, reads: Reads) -> Vec<String> {
    let mut findings = Vec::new();
    if unmanaged_but_present {
        findings.push(
            "the media server has this, but no service is monitoring it — it will not be \
             maintained, upgraded, or repaired if it is lost"
                .to_owned(),
        );
    }
    if !reads.history {
        findings.push(
            "this service's history could not be read, so how far the item got may be \
             understated — reported as unavailable, not read as nothing happened"
                .to_owned(),
        );
    }
    if !reads.queue {
        findings.push(
            "the download queue could not be read, so whether it is downloading now is \
             unknown — reported as unavailable, not read as stopped"
                .to_owned(),
        );
    }
    if !reads.parts {
        findings.push(
            "the episodes could not be read, so how much of this is here is unknown — \
             reported as unavailable, not read as a series with nothing in it"
                .to_owned(),
        );
    }
    findings
}

/// Why the item stopped where it did, in plain language — or `None` where nothing proves
/// it stopped. The generic reason a resting stage carries, sharpened by what only the live
/// reads can settle: a stuck queue names the download client; an import confirmed absent
/// from the library names the missing scan; downloading and beyond are otherwise either in
/// progress or beyond what the \*arr alone can judge.
fn stall_reason(
    furthest: Stage,
    queue_stuck: bool,
    library: Option<Presence>,
    reads: Reads,
) -> Option<String> {
    if queue_stuck {
        // The C7 signal: queued but not progressing — a real problem the operator can act
        // on, distinct from a download merely still running.
        return Some(
            "the download is in the queue but not progressing — the download client needs \
             attention"
                .to_owned(),
        );
    }
    // A stall claimed from an absence stands only where that absence was actually read.
    // Imported but confirmed absent from the library is provably awaiting a scan — a reason
    // only the media server can supply, so it stands only on a confirmed absence.
    match furthest {
        // Nobody asked — settled from the monitored flag alone, always known.
        Stage::NotMonitored => furthest.stall().map(str::to_owned),
        // Monitored and nothing since — but a "never found" is a claim about an empty
        // history, so only where the history was actually read, not where it could not be.
        Stage::Monitored if reads.history => furthest.stall().map(str::to_owned),
        // Grabbed and not in the queue — a claim about an empty queue, so only where the
        // queue was actually read.
        Stage::Grabbed if reads.queue => furthest.stall().map(str::to_owned),
        Stage::Imported if library == Some(Presence::Absent) => {
            Stage::Imported.stall().map(str::to_owned)
        }
        _ => None,
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
        history: Vec::new(),
        coverage: None,
        confidence: Confidence::Certain,
        findings: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::{assemble, library_presence, trace, Ctx, Fragments, Reads};
    use crate::config::Settings;
    use crate::jellyfin::Jellyfin;
    use crate::platform::Environment;
    use crate::ports::http::{Http, Request, Response, Unreachable};
    use crate::ports::service::{ItemPart, QueueItem, TraceEvent};
    use crate::recyclarr::Kind;
    use crate::stack::Source;
    use crate::test_support::{a_password, spoke, stack, Reporting, Scripted, SeedFs};
    use crate::trace::{Coverage, Outcome, Presence, Stage};

    /// A Servarr config that opens a target, carrying a readable key.
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

    /// An empty queue, as the shape a service returns with nothing downloading.
    const EMPTY_QUEUE: &str = r#"{"records":[]}"#;

    /// A series with no episodes listed — the shape that leaves a trace with no coverage
    /// to report, as a film's does.
    const NO_EPISODES: &str = "[]";

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
        episodes: &'static str,
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
                episodes: NO_EPISODES,
                sign_in: "",
                jellyfin_library: "",
            }
        }

        /// The same, with a series' episodes to aggregate.
        fn with_episodes(
            library: &'static str,
            queue: &'static str,
            episodes: &'static str,
        ) -> Self {
            Self {
                episodes,
                ..Self::arr(library, "{}", queue)
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
            } else if request.url.contains("/episode") {
                self.episodes
            } else {
                self.library
            };
            Ok(Response {
                status: 200,
                body: body.to_owned(),
            })
        }
    }

    fn event(outcome: Outcome) -> TraceEvent {
        TraceEvent {
            outcome,
            at: "2026-01-01T00:00:00Z".to_owned(),
        }
    }

    /// Fragments from services that all answered — the ordinary case the pure-assembly
    /// tests build on, with the read-failure flags set apart in their own tests.
    fn frags(
        events: Vec<TraceEvent>,
        queue: Vec<QueueItem>,
        library: Option<Presence>,
    ) -> Fragments {
        Fragments {
            events,
            queue,
            parts: Vec::new(),
            library,
            reads: Reads::ALL,
        }
    }

    /// One queue record for the item as a whole — a film's shape, naming no part.
    fn queued(stage: Stage, stuck: bool) -> Vec<QueueItem> {
        vec![QueueItem {
            part: None,
            stage,
            stuck,
        }]
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
            frags(
                vec![event(Outcome::Imported), event(Outcome::Grabbed)],
                Vec::new(),
                None,
            ),
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
        let report = assemble(
            "Sonarr",
            "The Expanse",
            true,
            frags(Vec::new(), Vec::new(), None),
        );
        assert_eq!(report.furthest, Stage::Monitored);
        assert!(report
            .stall
            .as_deref()
            .is_some_and(|reason| reason.contains("indexers returned nothing")));
    }

    #[test]
    fn a_repeated_attempt_shows_in_the_history_the_furthest_stage_flattens() {
        // Grabbed, the download failed, grabbed again: the furthest stage is a single
        // "grabbed" — a failure advances nothing — but the history keeps all three, oldest
        // first, so the repeated attempt is the pattern it is rather than one flat stage.
        let report = assemble(
            "Sonarr",
            "The Expanse",
            true,
            frags(
                vec![
                    event(Outcome::Grabbed),
                    event(Outcome::DownloadFailed),
                    event(Outcome::Grabbed),
                ],
                Vec::new(),
                None,
            ),
        );
        assert_eq!(report.furthest, Stage::Grabbed);
        let outcomes: Vec<Outcome> = report.history.iter().map(|moment| moment.outcome).collect();
        assert_eq!(
            outcomes,
            vec![Outcome::Grabbed, Outcome::DownloadFailed, Outcome::Grabbed]
        );
    }

    #[test]
    fn a_removal_after_an_import_shows_in_the_history() {
        // Imported then the file was removed: the history shows the full story including
        // the removal, though "removed" is not a stage the pipeline advances to, so the
        // furthest reached is still imported.
        let report = assemble(
            "Sonarr",
            "The Expanse",
            true,
            frags(
                vec![event(Outcome::Removed), event(Outcome::Imported)],
                Vec::new(),
                None,
            ),
        );
        assert_eq!(report.furthest, Stage::Imported);
        let outcomes: Vec<Outcome> = report.history.iter().map(|moment| moment.outcome).collect();
        assert_eq!(outcomes, vec![Outcome::Imported, Outcome::Removed]);
    }

    #[test]
    fn an_unmonitored_item_is_reported_as_nobody_asked() {
        let report = assemble(
            "Sonarr",
            "The Expanse",
            false,
            frags(Vec::new(), Vec::new(), None),
        );
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
            frags(vec![event(Outcome::Grabbed)], Vec::new(), None),
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
            frags(
                vec![event(Outcome::Grabbed)],
                queued(Stage::Downloading, false),
                None,
            ),
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
            frags(
                vec![event(Outcome::Grabbed)],
                queued(Stage::Downloading, true),
                None,
            ),
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
            frags(
                vec![event(Outcome::Imported)],
                Vec::new(),
                Some(Presence::Present),
            ),
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
            frags(
                vec![event(Outcome::Imported)],
                Vec::new(),
                Some(Presence::Absent),
            ),
        );
        assert_eq!(report.furthest, Stage::Imported);
        assert_eq!(report.confidence, crate::trace::Confidence::Certain);
        assert!(report
            .stall
            .as_deref()
            .is_some_and(|reason| reason.contains("has not been scanned")));
    }

    #[test]
    fn a_library_match_for_an_unmonitored_item_is_a_disagreement_not_availability() {
        // Nobody asked for it, yet the library has something by that title: the services
        // disagree. "Not monitored" is still how far it got — the stray match never
        // promotes it to available or marks the trace uncertain — but the contradiction is
        // surfaced as a finding rather than reconciled away.
        let report = assemble(
            "Sonarr",
            "The Expanse",
            false,
            frags(Vec::new(), Vec::new(), Some(Presence::Present)),
        );
        assert_eq!(report.furthest, Stage::NotMonitored);
        assert_eq!(report.confidence, crate::trace::Confidence::Certain);
        assert!(report
            .stall
            .as_deref()
            .is_some_and(|reason| reason.contains("nobody has asked")));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.contains("no service is monitoring it")));
    }

    #[test]
    fn agreeing_services_raise_no_disagreement() {
        // Monitored and present: the services agree, so there is no finding — a disagreement
        // is only raised where two views genuinely contradict.
        let report = assemble(
            "Sonarr",
            "The Expanse",
            true,
            frags(
                vec![event(Outcome::Imported)],
                Vec::new(),
                Some(Presence::Present),
            ),
        );
        assert_eq!(report.furthest, Stage::Available);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn an_unreadable_history_is_not_read_as_never_found() {
        // The history could not be read: an empty result must not be taken as "indexers
        // returned nothing" — the gap is reported as unavailable, not inferred as nothing.
        let report = assemble(
            "Sonarr",
            "The Expanse",
            true,
            Fragments {
                events: Vec::new(),
                queue: Vec::new(),
                parts: Vec::new(),
                library: None,
                reads: Reads {
                    history: false,
                    ..Reads::ALL
                },
            },
        );
        assert_eq!(report.furthest, Stage::Monitored);
        assert!(report.stall.is_none());
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.contains("history could not be read")));
    }

    #[test]
    fn an_unreadable_queue_does_not_prove_a_grab_stuck() {
        // Grabbed in history, but the queue could not be read: "the client never took it"
        // is a claim about an empty queue, so it is not made — the gap is reported instead.
        let report = assemble(
            "Sonarr",
            "The Expanse",
            true,
            Fragments {
                events: vec![event(Outcome::Grabbed)],
                queue: Vec::new(),
                parts: Vec::new(),
                library: None,
                reads: Reads {
                    queue: false,
                    ..Reads::ALL
                },
            },
        );
        assert_eq!(report.furthest, Stage::Grabbed);
        assert!(report.stall.is_none());
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.contains("queue could not be read")));
    }

    /// Every part still outstanding across the coverage, as season, number and stage —
    /// read by iteration because indexing a position the tests assume is barred.
    fn outstanding(coverage: &Coverage) -> Vec<(u32, u32, Stage)> {
        coverage
            .seasons
            .iter()
            .flat_map(|season| season.outstanding.iter())
            .map(|part| (part.season, part.number, part.stage))
            .collect()
    }

    /// One episode as its service records it.
    fn episode(id: i64, season: u32, number: u32, monitored: bool, has_file: bool) -> ItemPart {
        ItemPart {
            id,
            season,
            number,
            title: format!("S{season:02}E{number:02}"),
            monitored,
            has_file,
            grabbed: false,
        }
    }

    #[test]
    fn a_series_is_reported_season_by_season_not_by_one_episode_landing() {
        // The gap this closes: the history shows an import, so the item as a whole is
        // "imported" — which on its own reads as done while half the show is missing.
        let mut fragments = frags(vec![event(Outcome::Imported)], Vec::new(), None);
        fragments.parts = vec![
            episode(11, 1, 1, true, true),
            episode(12, 1, 2, true, false),
            episode(21, 2, 1, true, true),
            episode(22, 2, 2, true, true),
        ];
        let report = assemble("Sonarr", "The Expanse", true, fragments);
        assert_eq!(report.furthest, Stage::Imported);
        let coverage = report.coverage.unwrap_or_default();
        assert_eq!((coverage.have, coverage.wanted), (3, 4));
        assert!(!coverage.complete());
        // Season two is whole; season one is the one an operator would go and look at.
        let whole: Vec<bool> = coverage
            .seasons
            .iter()
            .map(crate::trace::SeasonCoverage::complete)
            .collect();
        assert_eq!(whole, vec![false, true]);
        assert_eq!(outstanding(&coverage), vec![(1, 2, Stage::Monitored)]);
    }

    #[test]
    fn an_episode_the_queue_is_downloading_is_not_a_stalled_grab() {
        // The join the coverage exists for. On the episode's own record it is grabbed and
        // nothing more, which reads as "the download client never took it" — but the queue
        // says it is downloading right now. Without the lift, every episode in flight
        // would report as a fault.
        let mut fragments = frags(
            Vec::new(),
            vec![QueueItem {
                part: Some(12),
                stage: Stage::Downloading,
                stuck: false,
            }],
            None,
        );
        let mut in_flight = episode(12, 1, 2, true, false);
        in_flight.grabbed = true;
        fragments.parts = vec![episode(11, 1, 1, true, true), in_flight];
        let report = assemble("Sonarr", "The Expanse", true, fragments);
        let coverage = report.coverage.unwrap_or_default();
        assert_eq!(outstanding(&coverage), vec![(1, 2, Stage::Downloading)]);
        // Downloading is work in progress, so it carries no stall reason at all.
        assert_eq!(Stage::Downloading.stall(), None);
    }

    #[test]
    fn a_grabbed_episode_the_queue_never_took_keeps_its_stall() {
        // The other side of the same join: grabbed, and the queue holds nothing for it.
        let mut fragments = frags(Vec::new(), Vec::new(), None);
        let mut lost = episode(12, 1, 2, true, false);
        lost.grabbed = true;
        fragments.parts = vec![lost];
        let report = assemble("Sonarr", "The Expanse", true, fragments);
        let coverage = report.coverage.unwrap_or_default();
        assert_eq!(outstanding(&coverage), vec![(1, 2, Stage::Grabbed)]);
        assert!(Stage::Grabbed
            .stall()
            .is_some_and(|reason| reason.contains("download client never took it")));
    }

    #[test]
    fn an_item_with_no_parts_reports_no_coverage() {
        // A film is the whole item — there is nothing to aggregate, so no coverage is
        // claimed rather than an empty one implying a series with nothing in it.
        let report = assemble(
            "Radarr",
            "Dune",
            true,
            frags(vec![event(Outcome::Imported)], Vec::new(), None),
        );
        assert_eq!(report.coverage, None);
    }

    #[test]
    fn unreadable_episodes_are_not_read_as_a_series_with_nothing_in_it() {
        let mut fragments = frags(Vec::new(), Vec::new(), None);
        fragments.reads.parts = false;
        let report = assemble("Sonarr", "The Expanse", true, fragments);
        assert_eq!(report.coverage, None);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.contains("episodes could not be read")));
    }

    #[test]
    fn a_queue_record_behind_the_history_is_not_shown_as_the_current_step() {
        // An item already imported, with a queue record still saying downloading — a
        // leftover the service has not cleared. Adding it would read as the item having
        // gone backwards, so the queue only shows where it carries the item forward.
        let report = assemble(
            "Sonarr",
            "The Expanse",
            true,
            frags(
                vec![event(Outcome::Imported)],
                queued(Stage::Downloading, false),
                None,
            ),
        );
        assert_eq!(report.furthest, Stage::Imported);
        assert_eq!(
            report.stages.last().map(|stage| stage.stage),
            Some(Stage::Imported)
        );
    }

    #[tokio::test]
    async fn an_unreadable_episode_listing_leaves_the_rest_of_the_trace_standing() {
        // The episodes will not read, so there is no coverage to report — but everything
        // the other services did answer still stands, and the gap is named.
        let context = ctx_with(Fake::with_episodes(
            r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
            EMPTY_QUEUE,
            "not json",
        ));
        let report = trace(&context, "expanse", None).await.unwrap_or_default();
        assert!(report.matched);
        assert_eq!(report.coverage, None);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.contains("episodes could not be read")));
    }

    #[tokio::test]
    async fn tracing_a_series_reads_its_episodes_into_coverage() {
        let context = ctx_with(Fake::with_episodes(
            r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
            EMPTY_QUEUE,
            r#"[
                {"id":11,"seasonNumber":1,"episodeNumber":1,"monitored":true,"hasFile":true},
                {"id":12,"seasonNumber":1,"episodeNumber":2,"monitored":true,"hasFile":false},
                {"id":13,"seasonNumber":0,"episodeNumber":1,"monitored":false,"hasFile":false}
            ]"#,
        ));
        let report = trace(&context, "expanse", None).await.unwrap_or_default();
        let coverage = report.coverage.unwrap_or_default();
        // The special nobody asked for is counted apart, never dragging the denominator
        // to three and reading as a fault to chase.
        assert_eq!((coverage.have, coverage.wanted), (1, 2));
        assert_eq!(coverage.unmonitored, 1);
    }

    #[tokio::test]
    async fn tracing_a_matched_item_reads_its_history_and_queue() {
        // Grabbed in history, and the queue shows it downloading — the trace reads both.
        let context = ctx(
            r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
            r#"{"records":[{"eventType":"grabbed","date":"2026-01-01T00:00:00Z"}]}"#,
            r#"{"records":[{"seriesId":1,"trackedDownloadState":"downloading","trackedDownloadStatus":"ok"}]}"#,
        );
        let report = trace(&context, "expanse", None).await.unwrap_or_default();
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
        let report = trace(&context, "expanse", None).await.unwrap_or_default();
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
        let report = trace(&context, "nothing here", None)
            .await
            .unwrap_or_default();
        assert!(!report.matched);
        assert_eq!(report.furthest, Stage::NotMonitored);
    }

    #[tokio::test]
    async fn tracing_passes_over_a_service_whose_library_cannot_be_read() {
        // A service that answers nonsense to the library read is passed over rather than
        // failing the whole trace; with every service unreadable, nothing matched.
        let report = trace(&ctx("not json", "{}", EMPTY_QUEUE), "expanse", None)
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
                episodes: NO_EPISODES,
                sign_in: SIGNED_IN,
                jellyfin_library: HAS_ITEM,
            },
            "available",
        );
        let report = trace(&context, "expanse", None).await.unwrap_or_default();
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
                episodes: NO_EPISODES,
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
        assert!(trace(&bad, "anything", None).await.is_err());
    }

    /// A queue holding one stuck download, the show embedded so it can be named.
    const STUCK_QUEUE: &str = r#"{"records":[{"trackedDownloadStatus":"warning","trackedDownloadState":"downloading","series":{"title":"The Expanse"}}]}"#;

    #[tokio::test]
    async fn stuck_lists_each_stuck_item_tagged_with_its_service() {
        // Sonarr's queue holds a stuck series; Radarr's holds nothing it can name (a series
        // record, no movie title) and Lidarr is not a traceable kind — so the one stuck
        // item is listed, tagged with the service holding it, and the list is complete.
        let report = super::stuck(&ctx("", "", STUCK_QUEUE))
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
        let report = super::stuck(&ctx("", "", "not json"))
            .await
            .unwrap_or_default();
        assert!(report.items.is_empty());
        assert!(report.incomplete);
    }

    #[tokio::test]
    async fn stuck_over_arrs_that_have_not_started_finds_nothing() {
        // No key is readable, so no \*arr opens: nothing was asked, so the list is empty
        // and complete rather than incomplete.
        let context = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            Settings::default(),
            Environment::MacOs,
        )
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)))
        .with_http(Arc::new(Fake::arr("", "", EMPTY_QUEUE)));
        let report = super::stuck(&context).await.unwrap_or_default();
        assert!(report.items.is_empty());
        assert!(!report.incomplete);
    }

    #[tokio::test]
    async fn a_stuck_query_over_an_unreadable_stack_is_an_error() {
        let bad = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            Source::External(std::path::Path::new("/lemonfiber/no/such/stack")),
            Settings::default(),
            Environment::MacOs,
        );
        assert!(super::stuck(&bad).await.is_err());
    }
}
