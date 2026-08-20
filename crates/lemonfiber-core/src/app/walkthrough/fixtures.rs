//! What the walkthrough's tests build their stacks out of.
//!
//! One transport answering every service the walk touches, because the walk touches all
//! of them in one run: a \*arr's catalogue, its root folders and profiles, the add, the
//! release probe, the history and the queue, and a media server's sign-in, library and
//! rescan. Splitting that across a fake each would mean a test setting up six things to
//! say one.

use std::sync::Arc;

use super::super::Ctx;
use crate::config::Settings;
use crate::platform::Environment;
use crate::ports::http::Method;
use crate::test_support::{a_password, spoke, stack, Reporting, Scripted, SeedFs};
use crate::walkthrough::{Line, Narrator};
use lemonfiber_fixtures::http::{Answer, Fake as Transport};

/// A Servarr configuration that opens a target, carrying a readable key.
const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

/// What each of the walk's requests is answered with. Every field is the raw body the
/// service would send, so a test says what a service said rather than what it meant.
#[derive(Clone)]
pub(super) struct Fake {
    /// What the catalogue lookup returns.
    pub lookup: &'static str,
    /// The root folders the service files under.
    pub folders: &'static str,
    /// The quality profiles it judges by.
    pub profiles: &'static str,
    /// The indexers it can search.
    pub indexers: &'static str,
    /// What the add answers with.
    pub added: &'static str,
    /// The wanted list a release probe starts from.
    pub wanted: &'static str,
    /// What a release search returns.
    pub releases: &'static str,
    /// The item's history.
    pub history: &'static str,
    /// The item's queue.
    pub queue: &'static str,
    /// Jellyfin's sign-in.
    pub sign_in: &'static str,
    /// Jellyfin's library.
    pub library: &'static str,
    /// What the download client says it is moving.
    pub transfers: &'static str,
    /// A path fragment every request to which fails, for the refusal paths.
    pub refuses: &'static str,
    /// Whether writes fail — the add, which is a write to the same path a lookup reads.
    pub refuses_writes: bool,
}

/// One indexer, enabled — a stack that can search.
pub(super) const ONE_INDEXER: &str = r#"[{"enableAutomaticSearch":true}]"#;

/// A catalogue result the service does not hold yet.
pub(super) const NOT_HELD: &str =
    r#"[{"id":0,"title":"Sintel","year":2010,"tvdbId":77,"tmdbId":99}]"#;

/// The same title, already in the library.
pub(super) const HELD: &str = r#"[{"id":7,"title":"Sintel","year":2010,"tvdbId":77,"tmdbId":99}]"#;

/// What an add answers with: the service's own id for what it took on.
pub(super) const ADDED: &str = r#"{"id":7,"title":"Sintel"}"#;

/// A wanted list with one missing item, so a release probe has something to search for.
pub(super) const ONE_WANTED: &str = r#"{"records":[{"id":11}]}"#;

/// A release the profile would grab.
pub(super) const A_RELEASE: &str = r#"[{"rejections":[]}]"#;

/// A history in which the item was imported.
pub(super) const IMPORTED: &str =
    r#"{"records":[{"eventType":"downloadFolderImported","date":"2026-08-08T00:00:00Z"}]}"#;

/// A download client with nothing in its queue.
pub(super) const NOTHING_MOVING: &str = r#"{"queue":{"kbpersec":"0","slots":[]}}"#;

/// A download client carrying the item, half done at fourteen megabytes a second.
pub(super) const CARRYING_IT: &str = r#"{"queue":{"kbpersec":"13672","slots":[{"filename":"Sintel.2010.1080p","percentage":"50","status":"Downloading","timeleft":"0:02:00","mbleft":"1050"}]}}"#;

/// The `SABnzbd` configuration that opens a download-client target, carrying a key.
pub(super) const SAB_KEYED: &str = "[misc]\napi_key = the-key\n";

/// A Jellyfin sign-in that hands back a token, and a library holding the item.
pub(super) const SIGNED_IN: &str = r#"{"AccessToken":"token"}"#;
pub(super) const HAS_ITEM: &str = r#"{"Items":[{"Name":"Sintel"}]}"#;
pub(super) const NO_ITEMS: &str = r#"{"Items":[]}"#;

impl Default for Fake {
    /// A stack where everything works and the item lands: the happy path, which each
    /// test then breaks in exactly one place.
    fn default() -> Self {
        Self {
            lookup: NOT_HELD,
            folders: r#"[{"path":"/data/media"}]"#,
            profiles: r#"[{"id":1}]"#,
            indexers: ONE_INDEXER,
            added: ADDED,
            wanted: ONE_WANTED,
            releases: A_RELEASE,
            history: IMPORTED,
            queue: r#"{"records":[],"totalRecords":0}"#,
            sign_in: SIGNED_IN,
            library: HAS_ITEM,
            transfers: NOTHING_MOVING,
            refuses: "\u{0}",
            refuses_writes: false,
        }
    }
}

/// What the world says this stack's address is — the same address the tunnel fixture has
/// both ends of the pair reporting, so nothing is leaking.
const EGRESS: &str = "203.0.113.7";

impl Fake {
    /// The transport this fake describes.
    ///
    /// A table rather than a chain of tests, because the chain was one long branch per
    /// service and read as a decision when it is really a lookup. Ordered most specific
    /// first: several of these paths are prefixes of each other, and a catalogue lookup is
    /// a `/series` request with more on the end.
    ///
    /// The refusals come first because they are about the request rather than the path —
    /// a write this stack will not take, or a path that answers nothing at all.
    pub(super) fn transport(&self) -> Arc<Transport> {
        let mut routes: Vec<(Option<Method>, &'static str, Answer)> = Vec::new();
        if self.refuses_writes {
            routes.push((Some(Method::Post), "", Answer::Silent));
        }
        routes.push((None, self.refuses, Answer::Silent));
        routes.extend(
            [
                ("echo.example", EGRESS),
                ("mode=queue", self.transfers),
                ("/AuthenticateByName", self.sign_in),
                ("/Library/Refresh", "{}"),
                ("/Items", self.library),
                ("lookup", self.lookup),
                ("/rootfolder", self.folders),
                ("/qualityprofile", self.profiles),
                ("/indexer", self.indexers),
                ("/wanted/missing", self.wanted),
                ("/release", self.releases),
                ("/history", self.history),
                ("/queue", self.queue),
                // Everything else is the add, which is a write to the library path itself.
                ("", self.added),
            ]
            .into_iter()
            .map(|(at, body)| (None, at, Answer::reply(200, body))),
        );
        Transport::by_rules(routes)
    }
}

/// A narrator that keeps what it was told, so a test can read the operator's own view of
/// the run rather than only its ending.
#[derive(Default)]
pub(super) struct Recording {
    said: std::sync::Mutex<Vec<Line>>,
}

impl Recording {
    /// Every line, in order.
    pub(super) fn lines(&self) -> Vec<Line> {
        self.said
            .lock()
            .map(|said| said.clone())
            .unwrap_or_default()
    }
}

impl Narrator for Recording {
    fn said(&self, line: &Line) {
        if let Ok(mut said) = self.said.lock() {
            said.push(line.clone());
        }
    }
}

/// A stack over the real manifest, a filesystem that opens the \*arrs, and a transport
/// answering as `fake` says — with no media server credential, so the library stage is
/// simply unreachable.
pub(super) fn ctx_with(fake: &Fake) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::absent()),
        Arc::new(crate::adapters::System),
        Arc::new(crate::adapters::Disk),
        stack(),
        over_usenet(),
        Environment::MacOs,
    )
    .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), Some(SAB_KEYED))))
    .with_http(fake.transport())
    // No waiting: every test would otherwise sit through the real poll, and what the wait
    // does at its bound is exactly what the tests are about.
    .waiting(std::time::Duration::ZERO)
}

/// The same, reachable media server and all — the admin password recorded under a scratch
/// environment file, tagged so each test keeps its own rather than racing on a shared one.
pub(super) fn ctx_watching(fake: &Fake, tag: &str) -> Ctx {
    let dir = std::env::temp_dir().join(format!("lemonfiber-walk-{tag}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let mut ctx = ctx_with(fake);
    ctx.settings.env_file = Some(dir.join(".env"));
    crate::app::targets::record_secret(
        &ctx,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        &a_password(),
    );
    ctx
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

/// Where the ticking clock starts — a plausible present, because the stack manifest is
/// validated against whatever day the clock says it is, and 1970 is not a day any stack
/// was released before.
const TODAY: std::time::Duration = std::time::Duration::from_secs(1_785_000_000);

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
    ctx.engine = Arc::new(
        Reporting::holding(
            &["gluetun", "qbittorrent"],
            crate::ports::docker::Lifecycle::Running,
            crate::ports::docker::Health::None,
        )
        .with_tunnel(crate::test_support::Tunnel {
            gateway: "gluetun",
            gateway_ip: Some("203.0.113.7"),
            client_ip: Some("203.0.113.7"),
            country: Some("nl"),
            port: Some("51413"),
            second_opinion: None,
        }),
    );
    ctx
}

/// A stack that acquires nothing, with a media server to ask about what it already has.
pub(super) fn ctx_library_only(fake: &Fake, tag: &str) -> Ctx {
    let mut ctx = ctx_watching(fake, tag);
    ctx.settings.protocols = acquires_nothing().protocols;
    ctx
}
