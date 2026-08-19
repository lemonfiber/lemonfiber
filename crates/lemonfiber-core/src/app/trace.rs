//! Following one item across the services, to answer "where is my show?".
//!
//! The question a household actually asks is answerable only by correlating fragments
//! four services each hold. This is the first cut: the \*arr that monitors an item and
//! records its history is the spine, and this reads it into one view — how far the item
//! got, and, where the \*arr's own record plainly shows it stopped, why.
//!
//! What the \*arr cannot see alone — whether an indexer found releases, whether the
//! download client took it, whether the media server can play it — is left to the
//! services that can; this never over-claims the reason for a stall it cannot prove.
//!
//! Two of those stalls are absences, and an absence has a cause no \*arr can see: nothing
//! found, and nothing taken. Both are what a lapsed account looks like from here — an
//! indexer with its allowance spent answers every search with an empty list, and an
//! account that is refusing or empty takes nothing it is handed. So where the trace stops
//! at one of those two, it asks the accounts, and what they say travels beside the stall
//! rather than replacing it: how far the item got is still the answer to the question
//! that was asked.
//!
//! Three questions, one per file: what each service holds, what the fragments amount to,
//! and why it stopped. The entry points stay here, because they are the errand.

mod assembling;
mod explaining;
mod reading;

use assembling::*;
use explaining::*;
use reading::*;

use std::sync::Arc;

use super::targets::{jellyfin_reader, open_servarrs};
use super::Ctx;
use crate::doctor::providers::ProvidersCheck;
use crate::doctor::{Check, Finding, Verdict};
use crate::error::{Diagnose, Problem};
use crate::model::{StuckEntry, StuckReport, TraceMoment, TraceReport, TraceStage};
use crate::ports::service::{
    Indexers, ItemPart, Library, Pipeline, QueueItem, TraceEvent, UsenetAccounts,
};
use crate::recyclarr::Kind;
use crate::trace::{Confidence, Coverage, Outcome, Part, Presence, Stage};

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
    // The media server is resolved once, ahead of the match: the last stage of a trace is
    // the same read whichever \*arr the item turns up in.
    let jellyfin = jellyfin_reader(ctx, &manifest.services);

    for arr in open_servarrs(ctx, &manifest.services).await {
        let (kind, service) = (arr.kind, arr.service);
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
        let mut report = assemble(
            &arr.name,
            &item.title,
            item.monitored,
            Fragments {
                events,
                queue,
                parts,
                library,
                reads,
            },
        );
        if let Some(reason) = account_explainable(&report) {
            let said = troubles(providers(ctx, &manifest.services).await);
            report.stall = Some(beside(reason, &said));
        }
        return Ok(report);
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
    let mut items = Vec::new();
    let mut incomplete = false;
    for arr in open_servarrs(ctx, &manifest.services).await {
        match arr.service.stuck_items(arr.kind).await {
            Ok(stuck) => items.extend(stuck.into_iter().map(|item| StuckEntry {
                title: item.title,
                service: arr.name.clone(),
                stage: item.stage,
            })),
            Err(_) => incomplete = true,
        }
    }
    Ok(StuckReport { items, incomplete })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lemonfiber_fixtures::http::{Answer, Fake as Transport};

    use super::{
        account_explainable, assemble, beside, library_presence, trace, troubles, Ctx, Finding,
        Fragments, Reads, TraceReport, Verdict,
    };
    use crate::config::Settings;
    use crate::doctor::Category;
    use crate::error::{Code, Problem, Remedy, Severity};
    use crate::jellyfin::Jellyfin;
    use crate::platform::Environment;
    use crate::ports::service::{ItemPart, QueueItem, TraceEvent};
    use crate::recyclarr::Kind;
    use crate::stack::Source;
    use crate::test_support::{a_password, spoke, stack, Reporting, Scripted, SeedFs};
    use crate::trace::{Coverage, Outcome, Presence, Stage};

    /// A Servarr config that opens a target, carrying a readable key.
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

    /// A download client configuration carrying the key it generated for itself.
    const SAB_INI: &str = "[misc]\napi_key = sabkey123\n";

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

    impl Fake {
        /// The scripted answers as a transport, routed by what each call asks for.
        fn transport(&self) -> Arc<Transport> {
            Transport::by_path(vec![
                ("/AuthenticateByName", Answer::reply(200, self.sign_in)),
                ("/Items", Answer::reply(200, self.jellyfin_library)),
                ("/history", Answer::reply(200, self.history)),
                ("/queue", Answer::reply(200, self.queue)),
                ("/episode", Answer::reply(200, self.episodes)),
                ("", Answer::reply(200, self.library)),
            ])
        }
    }

    fn event(outcome: Outcome) -> TraceEvent {
        TraceEvent {
            outcome,
            at: "2026-01-01T00:00:00Z".to_owned(),
            part: None,
        }
    }

    /// The same, recorded against one part — an episode's own history event.
    fn part_event(outcome: Outcome, part: i64) -> TraceEvent {
        TraceEvent {
            part: Some(part),
            ..event(outcome)
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
    fn ctx_with(fake: &Fake) -> Ctx {
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
        .with_http(fake.transport())
    }

    /// A context whose media server the trace cannot ask — no admin password is recorded,
    /// so the library stage is simply left unanswered, as on the \*arr-only slices.
    fn ctx(library: &'static str, history: &'static str, queue: &'static str) -> Ctx {
        ctx_with(&Fake::arr(library, history, queue))
    }

    /// A context whose download client's key is readable as well as the \*arrs', so a
    /// trace that asks the accounts resolves both readers rather than only one.
    fn ctx_with_accounts(fake: Fake) -> Ctx {
        ctx_with(&fake).with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), Some(SAB_INI))))
    }

    /// A context that can reach its Jellyfin: the admin password is recorded under the
    /// env file, so the trace's `jellyfin_reader` resolves a reading client. Tagged so
    /// each test keeps its own env file rather than racing on a shared one.
    fn ctx_with_jellyfin(fake: &Fake, tag: &str) -> Ctx {
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
    fn jellyfin(fake: &Fake) -> Jellyfin {
        Jellyfin::authenticated(
            fake.transport(),
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
            vec![part_event(Outcome::Grabbed, 12)],
            vec![QueueItem {
                part: Some(12),
                stage: Stage::Downloading,
                stuck: false,
            }],
            None,
        );
        fragments.parts = vec![
            episode(11, 1, 1, true, true),
            episode(12, 1, 2, true, false),
        ];
        let report = assemble("Sonarr", "The Expanse", true, fragments);
        let coverage = report.coverage.unwrap_or_default();
        assert_eq!(outstanding(&coverage), vec![(1, 2, Stage::Downloading)]);
        // Downloading is work in progress, so it carries no stall reason at all.
        assert_eq!(Stage::Downloading.stall(), None);
    }

    #[test]
    fn a_grabbed_episode_the_queue_never_took_keeps_its_stall() {
        // The other side of the same join: grabbed, and the queue holds nothing for it.
        let mut fragments = frags(vec![part_event(Outcome::Grabbed, 12)], Vec::new(), None);
        fragments.parts = vec![episode(12, 1, 2, true, false)];
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
        let context = ctx_with(&Fake::with_episodes(
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
        let context = ctx_with(&Fake::with_episodes(
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
            &Fake {
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
            Some(&jellyfin(&Fake {
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
            Some(&jellyfin(&Fake::arr("", "", ""))),
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
        .with_http(Fake::arr("", "", EMPTY_QUEUE).transport());
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

    /// A stall the accounts could explain, on a stack whose accounts will not answer: the
    /// reason stands exactly as it did. A service that could not be read says nothing about
    /// the accounts behind it, and a trace that added an empty aside would be claiming it
    /// had asked and heard something.
    #[tokio::test]
    async fn a_stall_reads_unchanged_where_the_accounts_cannot_be_read() {
        let context = ctx_with_accounts(Fake::arr(
            r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
            r#"{"records":[]}"#,
            EMPTY_QUEUE,
        ));
        let report = trace(&context, "expanse", None).await.unwrap_or_default();
        assert_eq!(report.furthest, Stage::Monitored);
        assert!(report.stall.as_deref().is_some_and(|reason| reason
            .contains("indexers returned nothing")
            && !reason.contains('(')));
    }

    /// The two stages an account can explain, and the ones it cannot. A preset that finds
    /// nothing good enough is not an account problem, and sending its operator to look at
    /// their subscriptions would be the wrong half of the answer.
    #[test]
    fn only_the_stalls_an_account_could_explain_ask_the_accounts() {
        let stalled = |furthest: Stage| TraceReport {
            furthest,
            stall: Some("stopped".to_owned()),
            ..TraceReport::default()
        };
        assert_eq!(
            account_explainable(&stalled(Stage::Monitored)).as_deref(),
            Some("stopped")
        );
        assert_eq!(
            account_explainable(&stalled(Stage::Grabbed)).as_deref(),
            Some("stopped")
        );
        assert_eq!(account_explainable(&stalled(Stage::Found)), None);
        assert_eq!(account_explainable(&stalled(Stage::Imported)), None);
        // Progressing rather than stopped: there is nothing to explain.
        assert_eq!(
            account_explainable(&TraceReport {
                furthest: Stage::Monitored,
                stall: None,
                ..TraceReport::default()
            }),
            None
        );
    }

    /// An account that is fine, or one nothing could be read from, explains nothing about
    /// why an item stopped — and saying so beside a stall would bury the reason.
    #[test]
    fn only_the_findings_that_want_acting_on_are_carried() {
        let said = troubles(vec![
            finding(
                "Fast Indexer",
                Verdict::Warn(problem("An indexer has used everything it allows for now")),
            ),
            finding(
                "Block 500",
                Verdict::Fail(problem("A Usenet account is refusing the login")),
            ),
            finding("Quiet", Verdict::Pass { note: None }),
            finding(
                "Unread",
                Verdict::Unverified {
                    reason: "nothing answered".to_owned(),
                    remedy: Remedy::new("try again"),
                },
            ),
        ]);
        assert_eq!(
            said,
            vec![
                "Fast Indexer — An indexer has used everything it allows for now".to_owned(),
                "Block 500 — A Usenet account is refusing the login".to_owned(),
            ]
        );
    }

    /// Beside rather than instead: how far the item got is still the answer to the
    /// question that was asked.
    #[test]
    fn what_the_accounts_say_travels_beside_the_stall() {
        let reason = "monitored, but no search has found it yet".to_owned();
        assert_eq!(beside(reason.clone(), &[]), reason);
        assert_eq!(
            beside(
                reason,
                &["Fast — capped".to_owned(), "Slow — refused".to_owned()]
            ),
            "monitored, but no search has found it yet (Fast — capped; Slow — refused)"
        );
    }

    /// A finding as the provider check reports one.
    fn finding(title: &str, verdict: Verdict) -> Finding {
        Finding::in_category(Category::Providers, "providers.test", title, verdict)
    }

    /// A problem whose summary is what a stall would quote.
    fn problem(summary: &str) -> Problem {
        Problem::new(
            Code::new("PROVIDER-1"),
            Severity::Warning,
            summary,
            "why it matters",
            Remedy::new("do something"),
        )
    }
}
