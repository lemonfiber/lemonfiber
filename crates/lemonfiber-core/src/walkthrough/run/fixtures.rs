//! What the walkthrough's tests build their stacks out of.
//!
//! One transport answering every service the walk touches, because the walk touches all
//! of them in one run: each curator's catalogue, how it would add, the add, the release
//! probe, the history and the queue, each over `library.curate` through the adapter in
//! front of it, and a media server's library and rescan. Splitting that across a fake
//! each would mean a test setting up six things to say one.

use std::sync::Arc;

use crate::app::Ctx;
use crate::config::Settings;
use crate::ports::docker::{Health, Lifecycle};
use crate::ports::service::{
    AddPlan, Added, CatalogueEntry, Download, QueueItem, ReleaseProbe, TraceEvent,
};
use crate::test_support::adapted::{running, Adapted};
use crate::test_support::curators::{answered, curated, FILM, TELEVISION};
use crate::test_support::{a_context, json, media, usenet, Reporting, SeedFs};
use crate::trace::{Outcome, Stage};
use crate::walkthrough::{Line, Narrator};
use lemonfiber_fixtures::http::{Answer, Fake as Transport};

/// What each of the walk's requests is answered with, as the adapter in front of each
/// curator answers it, so a test says what a curator said rather than what it meant.
#[derive(Clone)]
pub(super) struct Fake {
    /// What the catalogue lookup answers.
    pub lookup: Answer,
    /// What asking how an add would be filed answers.
    pub plan: Answer,
    /// How many indexers each curator searches.
    pub indexers: usize,
    /// What the add answers.
    pub added: Answer,
    /// What the release probe answers.
    pub releases: Answer,
    /// The item's history.
    pub history: Vec<TraceEvent>,
    /// The item's queue.
    pub queue: Vec<QueueItem>,
    /// Whether the media server's library holds the item.
    pub holds: bool,
    /// What the Usenet client says it is moving.
    pub transfers: Vec<Download>,
}

/// A catalogue entry for the title, held in the library as `held_as` where it is.
pub(super) fn sintel(held_as: Option<i64>) -> CatalogueEntry {
    CatalogueEntry {
        title: "Sintel".to_owned(),
        year: Some(2010),
        reference: 77,
        held_as,
    }
}

/// A catalogue answering `entries`.
pub(super) fn listing(entries: &[CatalogueEntry]) -> Answer {
    answered(&entries)
}

/// A history event of `outcome`.
pub(super) fn event(outcome: Outcome) -> TraceEvent {
    TraceEvent {
        outcome,
        at: "2026-08-08T00:00:00Z".to_owned(),
        part: None,
    }
}

/// A queue holding the item downloading, stuck where `stuck`.
pub(super) fn downloading(stuck: bool) -> Vec<QueueItem> {
    vec![QueueItem {
        part: None,
        stage: Stage::Downloading,
        stuck,
    }]
}

/// A Usenet client carrying the item, half done at fourteen megabytes a second with
/// 1050 mebibytes left.
pub(super) fn carrying_it() -> Vec<Download> {
    vec![Download {
        name: "Sintel.2010.1080p".to_owned(),
        progress: 50,
        speed: Some(13_672 * 1024),
        eta: Some(std::time::Duration::from_secs(120)),
        remaining: Some(1050 * 1024 * 1024),
    }]
}

impl Default for Fake {
    /// A stack where everything works and the item lands: the happy path, which each
    /// test then breaks in exactly one place.
    fn default() -> Self {
        Self {
            lookup: listing(&[sintel(None)]),
            plan: answered(&AddPlan {
                root_folder: "/data/media".to_owned(),
                quality_profile: 1,
            }),
            indexers: 1,
            added: answered(&Added {
                id: 7,
                title: "Sintel".to_owned(),
            }),
            releases: answered(&ReleaseProbe::Matching),
            history: vec![event(Outcome::Imported)],
            queue: Vec::new(),
            holds: true,
            transfers: Vec::new(),
        }
    }
}

/// What the world says this stack's address is — the same address the tunnel fixture has
/// both ends of the pair reporting, so nothing is leaking.
const EGRESS: &str = "203.0.113.7";

impl Fake {
    /// The transport this fake describes, every curator answering alike.
    pub(super) fn transport(&self) -> Arc<Transport> {
        Transport::by_path(vec![
            (
                usenet::operation("transfers"),
                Answer::reply(200, json(&self.transfers)),
            ),
            ("echo.example", Answer::reply(200, EGRESS)),
            (
                media::serve("has_item"),
                Answer::reply(200, json(&self.holds)),
            ),
            (media::serve("rescan"), Answer::reply(200, "null")),
            (curated("lookup"), self.lookup.clone()),
            (curated("add_plan"), self.plan.clone()),
            (curated("indexer_count"), answered(&self.indexers)),
            (curated("add"), self.added.clone()),
            (curated("probe_releases"), self.releases.clone()),
            (curated("item_history"), answered(&self.history)),
            (curated("item_queue"), answered(&self.queue)),
        ])
    }
}

/// A narrator that keeps what it was told, so a test can read the operator's own view of
/// the run rather than only its ending.
#[derive(Default)]
pub(super) struct Listening {
    said: std::sync::Mutex<Vec<Line>>,
}

impl Listening {
    /// Every line, in order.
    pub(super) fn lines(&self) -> Vec<Line> {
        self.said
            .lock()
            .map(|said| said.clone())
            .unwrap_or_default()
    }
}

impl Narrator for Listening {
    fn said(&self, line: &Line) {
        if let Ok(mut said) = self.said.lock() {
            said.push(line.clone());
        }
    }
}

/// The curators every stack here files television and film with, moved into their
/// plugins.
const CURATORS: [Adapted; 2] = [TELEVISION, FILM];

/// A stack over the real manifest whose curators have moved into their plugins and are
/// reached, with `also` moved in and reached beside them, the engine holding as well
/// whatever `engine` holds, and a transport answering as `fake` says.
fn built(fake: &Fake, also: &[Adapted], engine: Reporting) -> Ctx {
    let mut ctx = of_its_own(
        a_context()
            .settings(over_usenet())
            .build()
            .with_filesystem(Arc::new(SeedFs::keyed(None)))
            .with_http(fake.transport())
            .with_patience(std::time::Duration::ZERO),
    );
    let parts: Vec<Adapted> = CURATORS.iter().chain(also).copied().collect();
    ctx.seams.engine = Arc::new(running(&parts).alongside(engine));
    for part in &parts {
        part.moved_in(&ctx);
    }
    ctx
}

/// Nothing running besides the parts a stack here has moved in.
fn nothing_else() -> Reporting {
    Reporting::holding(&[], Lifecycle::Running, Health::Healthy)
}

/// A stack whose curators are reached, with its media server not moved into its plugin,
/// so the library stage is simply unreachable.
pub(super) fn ctx_with(fake: &Fake) -> Ctx {
    built(fake, &[], nothing_else())
}

/// `ctx` holding an env file of its own, in a scratch directory no other context shares.
///
/// Counted rather than named by the test that asked, because a name is an agreement the
/// next test has to keep and cannot be made to. Two contexts under one name share one
/// register, and recording a move rewrites it in place: the write truncates before it
/// writes, so a walk reading the register in that gap finds no plugin, opens no media
/// server, and stops at [`crate::walkthrough::Reason::NoMediaServer`] on a stack that has
/// one. Counting leaves nothing to agree about.
fn of_its_own(mut ctx: Ctx) -> Ctx {
    static BUILT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nth = BUILT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("walkthrough-{nth}")).kept();
    let _ = std::fs::create_dir_all(&dir);
    ctx.settings.env_file = Some(dir.join(".env"));
    ctx
}

/// The same, reachable media server and all, moved into its plugin.
pub(super) fn ctx_watching(fake: &Fake) -> Ctx {
    built(fake, &[media::MOVED], nothing_else())
}

/// The same, its Usenet client moved into its plugin and reachable, saying it moves what
/// `fake` says.
pub(super) fn ctx_downloading(fake: &Fake) -> Ctx {
    built(fake, &[usenet::MOVED], nothing_else())
}

/// Settings that acquire content over usenet — a configured stack with no tunnel to
/// prove, which is what most of these tests want out of the way.
pub(super) fn over_usenet() -> Settings {
    Settings {
        protocols: crate::config::Protocols {
            usenet: true,
            torrent: false,
        },
        ..Settings::default()
    }
}

/// Settings that acquire nothing — the library-only household.
pub(super) fn acquires_nothing() -> Settings {
    Settings {
        protocols: crate::config::Protocols {
            usenet: false,
            torrent: false,
        },
        ..Settings::default()
    }
}

/// A clock that moves on every reading, so a bounded wait reaches its bound without
/// anything actually waiting. The step is larger than nothing and smaller than the
/// patience the tests set, which is what makes a loop run more than once and then stop.
pub(super) struct Ticking {
    step: std::time::Duration,
    readings: std::sync::atomic::AtomicU64,
}

/// Where the ticking clock starts: the day every test validating the stack shares.
const TODAY: std::time::Duration =
    std::time::Duration::from_secs(lemonfiber_fixtures::ports::TODAY);

impl Ticking {
    /// A clock moving `step` further on each reading.
    pub(super) const fn by(step: std::time::Duration) -> Self {
        Self {
            step,
            readings: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

impl crate::ports::time::Clock for Ticking {
    fn now(&self) -> std::time::SystemTime {
        let taken = self
            .readings
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::time::UNIX_EPOCH + TODAY + self.step * u32::try_from(taken).unwrap_or(u32::MAX)
    }
}

/// A stack with torrents configured, which is the one that has a tunnel to prove.
pub(super) fn ctx_with_torrents(fake: &Fake) -> Ctx {
    let mut ctx = ctx_with(fake);
    ctx.settings.protocols = crate::config::Protocols::both();
    ctx
}

/// A stack with torrents configured whose tunnel genuinely holds — matching egress on
/// the pair the stack declares, which is what makes the gate open rather than stop.
pub(super) fn ctx_through_a_tunnel(fake: &Fake) -> Ctx {
    let mut ctx = ctx_with_torrents(fake);
    // What the check needs before it can say anything: somewhere to ask what address the
    // world sees, and a forwarding choice to judge.
    ctx.settings.ip_echo = vec!["https://echo.example".to_owned()];
    ctx.settings.port_forward = crate::config::PortForward {
        enabled: true,
        provider: Some("proton".to_owned()),
    };
    ctx.seams.engine = Arc::new(
        Reporting::holding(
            &["gluetun", "qbittorrent"],
            Lifecycle::Running,
            Health::None,
        )
        .with_tunnel(crate::test_support::Tunnel {
            gateway: "gluetun",
            gateway_ip: Some("203.0.113.7"),
            client_ip: Some("203.0.113.7"),
            country: Some("nl"),
            port: Some("51413"),
            second_opinion: None,
        })
        .alongside(running(&CURATORS)),
    );
    ctx
}

/// A stack that acquires nothing, with a media server to ask about what it already has.
pub(super) fn ctx_library_only(fake: &Fake) -> Ctx {
    let mut ctx = ctx_watching(fake);
    ctx.settings.protocols = acquires_nothing().protocols;
    ctx
}
