use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::{
    account_explainable, assemble, beside, library_presence, trace, troubles, Ctx, Fragments,
    Reads, TraceReport,
};
use crate::doctor::{Category, Finding, Verdict};
use crate::error::{Problem, Remedy};
use crate::ports::service::{FoundItem, ItemPart, QueueItem, StuckItem, TraceEvent};
use crate::recyclarr::Kind;
use crate::test_support::curators::{self, answered, curated, curating, refusing, TELEVISION};
use crate::test_support::{a_context, nowhere};
use crate::trace::{Coverage, Outcome, Presence, Stage};

/// A transport answering every curator's `library.curate` reads, so a trace's calls need
/// no exact ordering: what the library holds of a term, an item's history, queue and
/// parts, what is stuck, whether anything is wanted and what a release search for it
/// came to, and the media server's answer about its library.
struct Fake {
    items: Answer,
    history: Answer,
    queue: Answer,
    parts: Answer,
    stuck: Answer,
    holds: Answer,
    wanting: Answer,
    releases: Answer,
}

impl Fake {
    /// A transport answering the curators' reads, with nothing wanted and no media server
    /// configured to answer.
    fn curator(items: Answer, history: Answer, queue: Answer) -> Self {
        Self {
            items,
            history,
            queue,
            parts: answered(&Vec::<ItemPart>::new()),
            stuck: answered(&Vec::<StuckItem>::new()),
            holds: refusing(),
            wanting: answered(&false),
            releases: refusing(),
        }
    }

    /// The same, with the two reads a search against the indexers makes: whether anything
    /// is wanted, and what a release search for it came to.
    fn searching(items: Answer, wanting: bool, releases: Answer) -> Self {
        Self {
            wanting: answered(&wanting),
            releases,
            ..Self::curator(items, nothing_happened(), nothing_queued())
        }
    }

    /// The same, with a series' episodes to aggregate.
    fn with_episodes(items: Answer, queue: Answer, parts: Answer) -> Self {
        Self {
            parts,
            ..Self::curator(items, nothing_happened(), queue)
        }
    }

    /// The scripted answers as a transport, routed by what each call asks for.
    fn transport(&self) -> Arc<Transport> {
        Transport::by_path(vec![
            (
                crate::test_support::media::serve("has_item"),
                self.holds.clone(),
            ),
            (curated("find_items"), self.items.clone()),
            (curated("item_history"), self.history.clone()),
            (curated("item_queue"), self.queue.clone()),
            (curated("item_parts"), self.parts.clone()),
            (curating(TELEVISION, "stuck_items"), self.stuck.clone()),
            (curated("stuck_items"), answered(&Vec::<StuckItem>::new())),
            (curated("wanting"), self.wanting.clone()),
            (curated("probe_releases"), self.releases.clone()),
        ])
    }
}

/// The library holding one monitored item, with nothing yet done about it.
fn monitored() -> Answer {
    answered(&[FoundItem {
        id: 1,
        title: "The Expanse".to_owned(),
        year: None,
        monitored: true,
    }])
}

/// A history with nothing in it.
fn nothing_happened() -> Answer {
    answered(&Vec::<TraceEvent>::new())
}

/// A queue holding nothing.
fn nothing_queued() -> Answer {
    answered(&Vec::<QueueItem>::new())
}

/// A history holding `outcome`.
fn happened(outcome: Outcome) -> Answer {
    answered(&[event(outcome)])
}

fn event(outcome: Outcome) -> TraceEvent {
    TraceEvent {
        outcome,
        at: "2026-01-01T00:00:00Z".to_owned(),
        part: None,
    }
}

/// Fragments from services that all answered — the ordinary case the pure-assembly
/// tests build on, with the read-failure flags set apart in their own tests.
fn frags(events: Vec<TraceEvent>, queue: Vec<QueueItem>, library: Option<Presence>) -> Fragments {
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

/// A context over the real stack with an env file of its own, and a transport answering
/// the given reads.
fn unmoved(fake: &Fake) -> Ctx {
    static BUILT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nth = BUILT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("trace-{nth}")).kept();
    let _ = std::fs::create_dir_all(&dir);
    let mut ctx = a_context().build().with_http(fake.transport());
    ctx.settings.env_file = Some(dir.join(".env"));
    ctx
}

/// The same, with every curator moved into its plugin and reached through its adapter.
fn ctx_with(fake: &Fake) -> Ctx {
    curators::reaching(unmoved(fake), &curators::ALL)
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

/// A trace that was allowed to ask the indexers, and what it came back saying.
async fn searched_for(fake: &Fake) -> TraceReport {
    trace(&ctx_with(fake), "expanse", None, true)
        .await
        .unwrap_or_default()
}

/// A finding as the provider check reports one.
fn finding(title: &str, verdict: Verdict) -> Finding {
    Finding::in_category(Category::Providers, "providers.test", title, verdict)
}

mod reading;
mod searching;
mod seasons;
mod stages;
