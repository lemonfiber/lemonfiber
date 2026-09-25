use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::{
    account_explainable, assemble, beside, library_presence, trace, troubles, Ctx, Fragments,
    Reads, TraceReport,
};
use crate::doctor::{Category, Finding, Verdict};
use crate::error::{Problem, Remedy, Severity};
use crate::jellyfin::Jellyfin;
use crate::ports::service::{ItemPart, QueueItem, TraceEvent};
use crate::recyclarr::Kind;
use crate::test_support::{a_context, a_password, nowhere, SeedFs};
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
    wanted: &'static str,
    releases: &'static str,
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
            wanted: "",
            releases: "",
        }
    }

    /// The same, with the two reads a search against the indexers makes: the list
    /// of what is missing, and what a release search for the first of it returned.
    fn searching(library: &'static str, wanted: &'static str, releases: &'static str) -> Self {
        Self {
            wanted,
            releases,
            ..Self::arr(library, r#"{"records":[]}"#, EMPTY_QUEUE)
        }
    }

    /// The same, with a series' episodes to aggregate.
    fn with_episodes(library: &'static str, queue: &'static str, episodes: &'static str) -> Self {
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
            ("/wanted", Answer::reply(200, self.wanted)),
            ("/release", Answer::reply(200, self.releases)),
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

/// A context over the real stack, a filesystem that opens the \*arrs, and a transport
/// answering the given reads.
fn ctx_with(fake: &Fake) -> Ctx {
    a_context()
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .with_http(fake.transport())
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
