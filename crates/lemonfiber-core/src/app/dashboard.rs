//! Assembling one dashboard snapshot from what the ports can be reached for.
//!
//! The shape of the screen and the rules that keep it honest are the pure
//! [`crate::dashboard`] module; this is the driver that fills it from the live
//! stack. It never fails — a dashboard degrades rather than errors (a source that
//! cannot be reached marks its own panel and leaves the rest), so there is no
//! error channel through which a dead source could terminate the render loop.
//!
//! This gatherer fills every read-only panel: the services and their health, the
//! storage volume (free space, hardlink status, projected exhaustion), each \*arr's
//! queue, each download client's active transfers, and the VPN tunnel's state. What
//! remains is the ratatui surface that renders all of it on a refresh loop.

use std::path::Path;

use lemonfiber_manifest::Manifest;

use crate::app::Ctx;
use crate::dashboard::{
    eta, Hardlink, Panel, Protocol, Queue, Reading, Snapshot, Storage, Telemetry, Transfer, Vpn,
};
use crate::docker::{survey, Service};
use crate::doctor::vpn::{read_vpn, VpnReading};
use crate::error::Diagnose;
use crate::health::{observed, Egress, Reach, Summary};

use super::conditions;
use crate::ports::service::Queues;
use crate::storage::{test_link, Linked};

use super::targets::{
    download_targets, project_directory, read_transfers, servarr_targets, DownloadKind,
};

/// Gather one snapshot of what the stack is doing right now.
///
/// Reads the services through the engine; a stack that cannot be reached leaves
/// their panel unavailable and the telemetry disconnected. The panels without a
/// gatherer are marked pending — they do not drag the telemetry down, because a
/// panel nobody has wired is not a source that failed.
///
/// The health summary is not read from the panels but computed from what they
/// found, through the one shared computation every surface uses. So the tunnel is
/// read before the summary rather than beside it: a stack whose containers are all
/// healthy while its download client's traffic leaves outside the tunnel is the
/// case this ordering exists for.
pub async fn gather(ctx: &Ctx) -> Snapshot {
    let configured = ctx.settings.data_root.is_some();
    // The manifest every stack-derived panel reads from, resolved once — or the one
    // reason each reports if it cannot be read. Read once rather than re-parsed and
    // re-validated by each panel a second apart on the refresh loop, and so a stack
    // that cannot be read leaves every panel unavailable from the one failure.
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem().summary);
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());

    let seen = observe(ctx, manifest.as_ref()).await;
    let reach = Reach::of(configured, seen.as_deref().ok());
    let vpn = vpn(ctx, manifest.as_ref()).await;

    // A stack that could not be read has nothing to raise conditions about; the
    // summary then rests on the reach alone and says `unknown` rather than healthy.
    let health = summarise(
        ctx,
        reach,
        seen.as_deref().unwrap_or_default(),
        egress(vpn.as_ref()),
    );
    let services = match seen {
        Ok(services) => Panel::Ready(services),
        Err(reason) => Panel::unavailable(reason),
    };

    // Storage's exhaustion projection needs the rate downloads are landing on the
    // disk at, so the transfers are gathered first and their speeds carried into it.
    let transfers = transfers(ctx, manifest.as_ref(), project.as_deref()).await;
    let storage = storage(ctx, download_rate(&transfers)).await;

    Snapshot {
        // Only a source that genuinely failed marks the screen degraded; the
        // pending panels below are not-yet-built, not down, so they pass `false`.
        telemetry: Telemetry::read(reach, false),
        health,
        vpn,
        transfers,
        queue: queues(ctx, manifest.as_ref(), project.as_deref()).await,
        storage,
        services,
    }
}

/// Record what this refresh saw against the store the last one left, and
/// summarise from it.
///
/// Through the store rather than straight from the observations, because how long
/// a fault has lasted is the difference between a service that restarted once and
/// one that has been down all morning — and the summary grades them differently.
/// Read and written each refresh: the file is small, and a refresh that is not
/// remembered is one the next one has to guess about.
fn summarise(ctx: &Ctx, reach: Reach, services: &[Service], egress: Egress) -> Summary {
    let now = ctx.stamp();
    let mut conditions = conditions::load(ctx);
    for (check, fault) in observed(services, egress) {
        conditions.observe(&check, fault.as_ref(), &now);
    }
    // Everything the store knows, not only what is raised: a fault that has been
    // flapping is not called fixed the moment it blinks off.
    let summary = Summary::of(reach, &conditions.all(), &now);
    conditions::save(ctx, &conditions);
    summary
}

/// What the VPN panel proved about the download client's traffic.
///
/// A panel that could not be filled is unreadable rather than fine: the reason to
/// run a torrent client behind a tunnel is unverified, and the summary is entitled
/// to say so.
fn egress(vpn: Option<&Panel<Vpn>>) -> Egress {
    match vpn {
        None => Egress::NotApplicable,
        Some(Panel::Unavailable { .. }) => Egress::Unreadable,
        Some(Panel::Ready(vpn)) if vpn.egress_matches => Egress::Behind,
        Some(Panel::Ready(_)) => Egress::Leaking,
    }
}

/// The combined rate the active downloads are landing on the disk at — the sum of
/// the speeds actually reported this refresh, in bytes per second. A source that
/// went quiet contributes nothing rather than a guess, so a stalled queue projects
/// no exhaustion rather than a false one.
fn download_rate(transfers: &Panel<Vec<Transfer>>) -> u64 {
    match transfers {
        Panel::Ready(active) => active
            .iter()
            .filter_map(|transfer| match transfer.speed {
                Reading::Known(bytes) => Some(bytes),
                Reading::Stale(_) | Reading::Unknown => None,
            })
            .sum(),
        Panel::Unavailable { .. } => 0,
    }
}

/// The active downloads across the stack's download clients.
///
/// Resolves the download clients to host-side targets, then reads each on its own
/// shape — qBittorrent authenticated with the recorded password, `SABnzbd` with the
/// key it wrote to disk. A client not yet seeded (no password, or no key on disk)
/// or one that will not answer is left out rather than failing the panel; only a
/// stack that cannot be read at all leaves the whole panel unavailable, since then
/// there is nothing to ask. The protocol is set from which client answered, not
/// trusted from the answer.
async fn transfers(
    ctx: &Ctx,
    manifest: Result<&Manifest, &String>,
    project: Option<&Path>,
) -> Panel<Vec<Transfer>> {
    let manifest = match manifest {
        Ok(manifest) => manifest,
        Err(reason) => return Panel::unavailable(reason.clone()),
    };
    let targets = download_targets(&manifest.services, project);

    let mut active = Vec::new();
    for target in &targets {
        let downloads = read_transfers(ctx, target).await;
        let protocol = protocol_of(&target.kind);
        active.extend(downloads.into_iter().map(|download| Transfer {
            name: download.name,
            protocol,
            progress: download.progress,
            // A speed the client reported this refresh is known even at zero (a
            // stall); one it did not report is unknown, not a confident zero.
            speed: download.speed.map_or(Reading::Unknown, Reading::Known),
            eta: download.eta,
        }));
    }
    Panel::Ready(active)
}

/// The protocol a client's transfers move over.
fn protocol_of(kind: &DownloadKind) -> Protocol {
    match kind {
        DownloadKind::Qbittorrent => Protocol::Torrent,
        DownloadKind::Sabnzbd { .. } => Protocol::Usenet,
    }
}

/// What the VPN is doing, and whether the download client is genuinely behind it.
///
/// `None` where the stack has no VPN-contained torrent client — the panel does not
/// apply, rather than showing an empty box. Otherwise the tunnel's exit address,
/// country and forwarded port, and the egress-match that proves the client's
/// traffic leaves through it, or the reason none of that could be read. Reuses the
/// leak check's own containers and exec-reads ([`read_vpn`]) so the panel and the
/// diagnostic cannot disagree about what the tunnel is doing.
async fn vpn(ctx: &Ctx, manifest: Result<&Manifest, &String>) -> Option<Panel<Vpn>> {
    let manifest = match manifest {
        Ok(manifest) => manifest,
        Err(reason) => return Some(Panel::unavailable(reason.clone())),
    };
    let reading = read_vpn(
        ctx.engine.as_ref(),
        &ctx.settings.project,
        manifest,
        ctx.settings.protocols,
        ctx.settings.ip_echo.as_deref(),
        ctx.settings.port_forward.enabled,
    )
    .await;
    match reading {
        VpnReading::NotApplicable => None,
        VpnReading::Unavailable(reason) => Some(Panel::unavailable(reason)),
        VpnReading::Ready {
            exit_ip,
            country,
            forwarded_port,
            egress_matches,
        } => Some(Panel::Ready(Vpn {
            exit_ip,
            country: country.unwrap_or_default(),
            forwarded_port,
            egress_matches,
        })),
    }
}

/// Each media-filing \*arr's queue depth and stuck count.
///
/// Resolves the Servarr-shape services the same way the credentials check does —
/// the stack's own bind-mount convention — then reads each one's key from disk and
/// asks it for its queue. A service still starting (no key written yet) or one
/// that will not answer is left out of the panel rather than failing it; only a
/// stack that cannot be read at all leaves the whole panel unavailable, since then
/// there are no services to ask.
async fn queues(
    ctx: &Ctx,
    manifest: Result<&Manifest, &String>,
    project: Option<&Path>,
) -> Panel<Vec<Queue>> {
    let manifest = match manifest {
        Ok(manifest) => manifest,
        Err(reason) => return Panel::unavailable(reason.clone()),
    };
    let targets = servarr_targets(&manifest.services, project);

    let mut depths = Vec::new();
    for target in &targets {
        let Some(service) = target.open(&ctx.http, ctx.filesystem.as_ref()).await else {
            continue;
        };
        if let Ok(depth) = service.queue().await {
            depths.push(Queue {
                service: target.name.clone(),
                depth: depth.total,
                stuck: depth.stuck,
            });
        }
    }
    Panel::Ready(depths)
}

/// The storage picture: how much is free, whether imports link, and when it fills.
///
/// Free space is read afresh each refresh — a cheap read. A volume that could not
/// be attributed to any mount reports a zero total, and its free space is then
/// unknown rather than zero: "cannot read the volume" and "the disk is full" are
/// opposite things to an operator and must not render alike. The hardlink status
/// comes from the empirical probe, and the exhaustion from the free space against
/// the rate downloads are landing at (`download_rate`), so a stalled queue
/// projects no exhaustion rather than one that never arrives.
async fn storage(ctx: &Ctx, download_rate: u64) -> Panel<Storage> {
    let Some(root) = ctx.settings.data_root.as_deref() else {
        return Panel::unavailable("no data location is configured");
    };
    let facts = ctx.filesystem.describe(root).await;
    let free = if facts.total == 0 {
        Reading::Unknown
    } else {
        Reading::Known(facts.available)
    };
    // The hardlink test writes — it creates a file, links it, and inspects the two
    // names — unlike the cheap free-space read. Cheap once, but a per-refresh write;
    // the refresh loop will run it far less often, and until then it runs each time.
    let hardlink = hardlink_of(&test_link(ctx.filesystem.as_ref(), root).await);
    // Exhaustion is the free space divided by the rate it is draining at: a rate of
    // zero divides to no estimate rather than an infinite one, and a volume that
    // could not be read projects nothing rather than a wrong time.
    let exhaustion = match free {
        Reading::Known(bytes) => eta(bytes, download_rate),
        Reading::Stale(_) | Reading::Unknown => None,
    };
    Panel::Ready(Storage {
        free,
        exhaustion,
        hardlink,
    })
}

/// The dashboard's hardlink status from the empirical probe: it links, it copies,
/// or it could not be established — an unwritable location or an unconfirmed link
/// is never reported as a met guarantee.
fn hardlink_of(linked: &Linked) -> Hardlink {
    match linked {
        Linked::Yes { .. } => Hardlink::Linking,
        Linked::No => Hardlink::Copying,
        Linked::Unwritable { .. } | Linked::Unconfirmed => Hardlink::Unknown,
    }
}

/// Observe every service the stack declares, or the reason it could not be read.
///
/// The reason is the operator-facing summary of whatever went wrong — an
/// unreadable stack, an engine that would not answer — so the panel that carries
/// it says something an operator can act on rather than a bare failure.
async fn observe(ctx: &Ctx, manifest: Result<&Manifest, &String>) -> Result<Vec<Service>, String> {
    let manifest = manifest.map_err(Clone::clone)?;
    let profiles: Vec<String> = manifest
        .profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect();
    let containers = ctx
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|err| err.problem().summary)?;
    Ok(survey(manifest, &profiles, &containers))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;

    use super::{gather, vpn};
    use crate::app::Ctx;
    use crate::config::{PortForward, Protocols, Settings};
    use crate::dashboard::{Hardlink, Panel, Protocol, Reading, Telemetry, Transfer, Vpn};
    use crate::health::Standing;
    use crate::platform::Environment;
    use crate::ports::docker::{Health, Lifecycle};
    use crate::ports::filesystem::{FsKind, StorageFacts};
    use crate::ports::http::{Http, Request, Response, Unreachable};
    use crate::stack::Source;
    use crate::test_support::{
        a_password, spoke, stack, Reporting, Scripted, ScriptedHttp, SeedFs, Tunnel,
    };

    /// A transport that answers every request with the same body — a service's
    /// queue as JSON for the happy path, or something unreadable to stand in for a
    /// service that will not answer.
    struct HttpReturning(&'static str);

    #[async_trait]
    impl Http for HttpReturning {
        async fn send(&self, _request: &Request) -> Result<Response, Unreachable> {
            Ok(Response {
                status: 200,
                body: self.0.to_owned(),
            })
        }
    }

    /// A Servarr config carrying a usable key, and one carrying none.
    const CONFIG_WITH_KEY: &str = "<Config><ApiKey>a1b2c3d4e5</ApiKey></Config>";
    const CONFIG_NO_KEY: &str = "<Config><Port>8989</Port></Config>";

    /// A queue as a service reports it: four items, one of them stuck.
    const QUEUE_JSON: &str = r#"{"totalRecords":4,"records":[{"trackedDownloadStatus":"warning"},{"trackedDownloadStatus":"ok"}]}"#;

    /// Storage facts a volume with `total` bytes, `available` free, would report.
    fn facts(available: u64, total: u64) -> StorageFacts {
        StorageFacts {
            kind: FsKind::Linking("ext4".to_owned()),
            removable: false,
            available,
            total,
        }
    }

    /// A context whose engine reports whatever the test put in it, configured with
    /// a data root so it is not read as an unconfigured machine.
    fn ctx(engine: Reporting) -> Ctx {
        let settings = Settings {
            protocols: Protocols::both(),
            data_root: Some(std::path::PathBuf::from("/srv/media")),
            ..Settings::default()
        };
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(engine),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            settings,
            Environment::MacOs,
        )
        .waiting(Duration::ZERO)
    }

    /// Every service the `library` form declares.
    const LIBRARY: [&str; 4] = [
        "jellyfin",
        "seerr",
        "calibre-web-automated",
        "audiobookshelf",
    ];

    #[tokio::test]
    async fn a_running_stack_fills_the_services_and_health_and_reads_as_up() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
        let snapshot = gather(&ctx(engine)).await;

        assert!(
            matches!(snapshot.services, Panel::Ready(ref services) if !services.is_empty()),
            "the services panel is filled"
        );
        // The health summary is the stack's verdict, not the screen's: the services
        // the engine reports are healthy, so nothing wants attention — except that
        // this stack declares a torrent client whose egress cannot be read here, and
        // an unverified tunnel is a finding rather than silence.
        assert_eq!(snapshot.health.standing, Standing::Degraded);
        let affected: Vec<&str> = snapshot
            .health
            .affected
            .iter()
            .map(|item| item.check.as_str())
            .collect();
        assert_eq!(affected, vec!["vpn.egress"]);
        // Telemetry is how the screen is doing, not the stack: every source that has
        // a gatherer answered and the pending panels are not failures, so it reads
        // live even though the stack is only partly up.
        assert_eq!(snapshot.telemetry, Telemetry::Live);
    }

    #[tokio::test]
    async fn every_panel_now_has_a_gatherer_rather_than_reading_as_pending() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
        let snapshot = gather(&ctx(engine)).await;
        // The VPN panel is now gathered: this stack declares a torrent client, so it
        // applies (`Some`), and — with no IP-echo configured here — reports that its
        // egress cannot be read rather than being omitted.
        assert!(matches!(snapshot.vpn, Some(Panel::Unavailable { .. })));
        // The transfers panel is filled — empty here, since the real filesystem holds
        // no download-client credentials to read.
        assert!(matches!(snapshot.transfers, Panel::Ready(ref active) if active.is_empty()));
    }

    #[tokio::test]
    async fn an_idle_stack_reads_as_no_stack() {
        // Containers exist but none is running — configured, reachable, nothing up.
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Exited, Health::None);
        let snapshot = gather(&ctx(engine)).await;
        assert_eq!(snapshot.telemetry, Telemetry::NoStack);
        // An idle stack is stopped on purpose, not a failure.
        assert_eq!(snapshot.health.standing, Standing::Stopped);
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_also_leaves_the_dashboard_disconnected() {
        // The other way observing can fail: the stack itself is unreadable, before
        // the engine is even asked. The services panel carries that reason.
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let settings = Settings {
            protocols: Protocols::both(),
            data_root: Some(std::path::PathBuf::from("/srv/media")),
            ..Settings::default()
        };
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::holding(
                &LIBRARY,
                Lifecycle::Running,
                Health::Healthy,
            )),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            nowhere,
            settings,
            Environment::MacOs,
        );
        let snapshot = gather(&ctx).await;
        assert_eq!(snapshot.telemetry, Telemetry::Disconnected);
        assert!(!snapshot.services.is_available());
        assert!(
            !snapshot.queue.is_available(),
            "a stack that cannot be read has no services to ask for a queue"
        );
        assert!(
            !snapshot.transfers.is_available(),
            "nor any download client to ask for its transfers"
        );
    }

    /// A context configured with a fake filesystem and transport, over the stack
    /// this repo carries so its \*arr services resolve as queue targets.
    fn ctx_with(fs: SeedFs, http: HttpReturning) -> Ctx {
        ctx(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            Health::Healthy,
        ))
        .with_filesystem(Arc::new(fs))
        .with_http(Arc::new(http))
    }

    #[tokio::test]
    async fn the_queue_panel_fills_with_each_arrs_depth_and_stuck_count() {
        let ctx = ctx_with(
            SeedFs::keyed(Some(CONFIG_WITH_KEY), None),
            HttpReturning(QUEUE_JSON),
        );
        let snapshot = gather(&ctx).await;
        assert!(
            matches!(snapshot.queue, Panel::Ready(ref queues)
                if !queues.is_empty() && queues.iter().all(|q| q.depth == 4 && q.stuck == 1)),
            "each *arr that answered contributes its depth and stuck count"
        );
    }

    #[tokio::test]
    async fn a_service_still_starting_with_no_key_is_left_out_of_the_queue() {
        // No config to read: the ordinary first-start case, skipped so the panel is
        // ready-but-empty rather than failed.
        let ctx = ctx_with(SeedFs::keyed(None, None), HttpReturning(QUEUE_JSON));
        let snapshot = gather(&ctx).await;
        assert!(matches!(snapshot.queue, Panel::Ready(ref queues) if queues.is_empty()));
    }

    #[tokio::test]
    async fn a_service_whose_config_holds_no_key_is_left_out_of_the_queue() {
        let ctx = ctx_with(
            SeedFs::keyed(Some(CONFIG_NO_KEY), None),
            HttpReturning(QUEUE_JSON),
        );
        let snapshot = gather(&ctx).await;
        assert!(matches!(snapshot.queue, Panel::Ready(ref queues) if queues.is_empty()));
    }

    #[tokio::test]
    async fn a_service_that_will_not_answer_its_queue_is_left_out() {
        // The key reads, but the queue answer is unreadable, so that service is
        // dropped from the panel rather than failing it.
        let ctx = ctx_with(
            SeedFs::keyed(Some(CONFIG_WITH_KEY), None),
            HttpReturning("not a queue"),
        );
        let snapshot = gather(&ctx).await;
        assert!(matches!(snapshot.queue, Panel::Ready(ref queues) if queues.is_empty()));
    }

    #[tokio::test]
    async fn an_unreachable_engine_leaves_services_unavailable_and_reads_disconnected() {
        let snapshot = gather(&ctx(Reporting::absent())).await;
        assert_eq!(snapshot.telemetry, Telemetry::Disconnected);
        assert!(
            !snapshot.services.is_available(),
            "a stack that cannot be read leaves its services unavailable, not empty"
        );
        // The summary is still there, and says it does not know — never healthy, and
        // never absent, since a blank space is a reading an operator can misread.
        assert_eq!(snapshot.health.standing, Standing::Unknown);
    }

    #[tokio::test]
    async fn a_machine_with_no_data_root_reads_as_unconfigured() {
        let settings = Settings {
            protocols: Protocols::both(),
            data_root: None,
            ..Settings::default()
        };
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::holding(
                &LIBRARY,
                Lifecycle::Running,
                Health::Healthy,
            )),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            settings,
            Environment::MacOs,
        );
        let snapshot = gather(&ctx).await;
        assert_eq!(snapshot.telemetry, Telemetry::Unconfigured);
        assert_eq!(snapshot.health.standing, Standing::Unconfigured);
        assert!(
            !snapshot.storage.is_available(),
            "with no data location there is no volume to report free space on"
        );
    }

    #[tokio::test]
    async fn storage_reports_the_free_space_when_the_volume_reads() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
        let ctx = ctx(engine).with_filesystem(Arc::new(
            SeedFs::keyed(None, None).with_facts(facts(42, 100)),
        ));
        let snapshot = gather(&ctx).await;
        assert!(
            matches!(snapshot.storage, Panel::Ready(storage) if storage.free == Reading::Known(42)),
            "the volume's free space fills the panel"
        );
    }

    #[tokio::test]
    async fn an_unreadable_volume_reports_free_space_unknown_not_zero() {
        // A volume attributed to no mount reports a zero total; its free space is
        // unknown, not a confident zero that reads as a full disk.
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
        let ctx = ctx(engine)
            .with_filesystem(Arc::new(SeedFs::keyed(None, None).with_facts(facts(0, 0))));
        let snapshot = gather(&ctx).await;
        assert!(
            matches!(snapshot.storage, Panel::Ready(storage) if storage.free == Reading::Unknown),
            "a volume that could not be read reports unknown free space, not zero"
        );
    }

    /// A transport that answers each download client on its own path: qBittorrent's
    /// login and its torrent list, and — anything else being the only other call a
    /// read makes — `SABnzbd`'s queue.
    struct Downloads {
        torrents: &'static str,
        queue: &'static str,
    }

    #[async_trait]
    impl Http for Downloads {
        async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
            let body = if request.url.contains("/auth/login") {
                "Ok."
            } else if request.url.contains("/torrents/info") {
                self.torrents
            } else {
                self.queue
            };
            Ok(Response {
                status: 200,
                body: body.to_owned(),
            })
        }
    }

    /// One qBittorrent torrent, 30% done at a known speed with an ETA.
    const QBIT_TORRENTS: &str =
        r#"[{"name":"Ubuntu.iso","completed":300,"size":1000,"dlspeed":4096,"eta":120}]"#;
    /// One `SABnzbd` download, whose queue speed will not parse — so its speed reads
    /// unknown rather than a false zero.
    const SAB_QUEUE: &str = r#"{"queue":{"kbpersec":"nan","slots":[{"filename":"Linux.nzb","percentage":"20","status":"Downloading","timeleft":"0:05:00"}]}}"#;
    /// A `sabnzbd.ini` carrying a usable key, and one that has not written it yet.
    const SAB_KEY_INI: &str = "[misc]\napi_key = sabkey123\n";
    const SAB_NO_KEY_INI: &str = "[misc]\nhost = 0.0.0.0\n";

    /// A context configured to read download clients: the library stack running, a
    /// fake filesystem for `SABnzbd`'s key, the given transport, and — where set — an
    /// env file holding qBittorrent's recorded password.
    fn ctx_downloads(fs: SeedFs, http: Arc<dyn Http>, env_file: Option<PathBuf>) -> Ctx {
        let settings = Settings {
            protocols: Protocols::both(),
            data_root: Some(PathBuf::from("/srv/media")),
            env_file,
            ..Settings::default()
        };
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::holding(
                &LIBRARY,
                Lifecycle::Running,
                Health::Healthy,
            )),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            settings,
            Environment::MacOs,
        )
        .waiting(Duration::ZERO)
        .with_filesystem(Arc::new(fs))
        .with_http(http)
    }

    /// A private env file recording qBittorrent's password, at a scratch path
    /// unique to the test so concurrent tests do not share it.
    fn env_at(name: &str, password: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-dash-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(".env");
        assert!(
            crate::config::store::set(&path, crate::config::QBITTORRENT_PASSWORD_KEY, password)
                .is_ok(),
            "the scratch env file is written"
        );
        path
    }

    #[tokio::test]
    async fn the_transfers_panel_fills_from_each_download_client() {
        let http: Arc<dyn Http> = Arc::new(Downloads {
            torrents: QBIT_TORRENTS,
            queue: SAB_QUEUE,
        });
        let ctx = ctx_downloads(
            SeedFs::keyed(None, Some(SAB_KEY_INI)),
            http,
            Some(env_at("fills", &a_password())),
        );
        let snapshot = gather(&ctx).await;

        // The torrent client's download: progress from its byte counts, a known
        // speed even at a value, and its ETA — tagged as a torrent by which client
        // answered, not by anything the client said.
        assert!(
            matches!(&snapshot.transfers, Panel::Ready(active) if active.iter().any(|t|
                matches!(t.protocol, Protocol::Torrent)
                    && t.name == "Ubuntu.iso"
                    && t.progress == 30
                    && matches!(t.speed, Reading::Known(4096))
                    && t.eta == Some(Duration::from_secs(120)))),
            "the torrent client's download fills a torrent transfer"
        );
        // The Usenet client's download: a speed the client could not read is
        // unknown, not a confident zero that would read as a stall.
        assert!(
            matches!(&snapshot.transfers, Panel::Ready(active) if active.iter().any(|t|
                matches!(t.protocol, Protocol::Usenet)
                    && t.name == "Linux.nzb"
                    && t.progress == 20
                    && matches!(t.speed, Reading::Unknown)
                    && t.eta == Some(Duration::from_secs(300)))),
            "the Usenet client's download fills a Usenet transfer"
        );
        assert!(matches!(&snapshot.transfers, Panel::Ready(active) if active.len() == 2));
    }

    #[tokio::test]
    async fn a_client_not_yet_seeded_is_left_out_not_a_failure() {
        // No recorded qBittorrent password and no SABnzbd key on disk: both are
        // still finishing first start, so each is skipped and the panel is
        // ready-but-empty rather than failed.
        let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new(Vec::new()));
        let ctx = ctx_downloads(SeedFs::keyed(None, None), http, None);
        let snapshot = gather(&ctx).await;
        assert!(matches!(&snapshot.transfers, Panel::Ready(active) if active.is_empty()));
    }

    #[tokio::test]
    async fn a_client_whose_key_is_not_on_disk_yet_is_left_out() {
        // SABnzbd has written a config but not its key; qBittorrent has no recorded
        // password. Neither can be read, so neither appears.
        let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new(Vec::new()));
        let ctx = ctx_downloads(SeedFs::keyed(None, Some(SAB_NO_KEY_INI)), http, None);
        let snapshot = gather(&ctx).await;
        assert!(matches!(&snapshot.transfers, Panel::Ready(active) if active.is_empty()));
    }

    #[tokio::test]
    async fn a_download_client_that_will_not_answer_is_left_out() {
        // The password is recorded, but qBittorrent's login goes unanswered, so it
        // is dropped from the panel rather than failing it.
        let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new(Vec::new()));
        let ctx = ctx_downloads(
            SeedFs::keyed(None, None),
            http,
            Some(env_at("silent", &a_password())),
        );
        let snapshot = gather(&ctx).await;
        assert!(matches!(&snapshot.transfers, Panel::Ready(active) if active.is_empty()));
    }

    // ── VPN panel ─────────────────────────────────────────────────

    /// A healthy tunnel scripted onto the gluetun/qBittorrent pair the stack
    /// declares: matching egress, a country, and a forwarded port.
    fn healthy_tunnel() -> Tunnel {
        Tunnel {
            gateway: "gluetun",
            gateway_ip: Some("203.0.113.7"),
            client_ip: Some("203.0.113.7"),
            country: Some("nl"),
            port: Some("51413"),
        }
    }

    /// An engine holding the VPN pair in one lifecycle, optionally scripted to
    /// answer the probe.
    fn tunnel_engine(running: bool, tunnel: Option<Tunnel>) -> Reporting {
        let engine = Reporting::holding(
            &["gluetun", "qbittorrent"],
            if running {
                Lifecycle::Running
            } else {
                Lifecycle::Exited
            },
            Health::None,
        );
        match tunnel {
            Some(tunnel) => engine.with_tunnel(tunnel),
            None => engine,
        }
    }

    /// Port forwarding, enabled for a provider.
    fn forwarding() -> PortForward {
        PortForward {
            enabled: true,
            provider: Some("proton".to_owned()),
        }
    }

    /// A context that reads the VPN through the given engine, with leak detection
    /// (the IP-echo) and port forwarding as configured.
    fn vpn_ctx(
        engine: Reporting,
        ip_echo: Option<&str>,
        port_forward: PortForward,
        protocols: Protocols,
    ) -> Ctx {
        let settings = Settings {
            protocols,
            data_root: Some(PathBuf::from("/srv/media")),
            ip_echo: ip_echo.map(str::to_owned),
            port_forward,
            ..Settings::default()
        };
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(engine),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            settings,
            Environment::MacOs,
        )
        .waiting(Duration::ZERO)
    }

    /// The VPN panel the way `gather` reads it — the manifest resolved from the
    /// stack, handed to the driver.
    async fn vpn_panel(ctx: &Ctx) -> Option<Panel<Vpn>> {
        let manifest = ctx
            .stack
            .checked_manifest(ctx.today())
            .map_err(|err| crate::error::Diagnose::problem(&err).summary);
        vpn(ctx, manifest.as_ref()).await
    }

    #[tokio::test]
    async fn the_vpn_panel_shows_the_tunnel_when_it_answers() {
        let ctx = vpn_ctx(
            tunnel_engine(true, Some(healthy_tunnel())),
            Some("https://echo"),
            forwarding(),
            Protocols::both(),
        );
        assert!(
            matches!(vpn_panel(&ctx).await, Some(Panel::Ready(v))
                if v.exit_ip == "203.0.113.7"
                    && v.country == "NL"
                    && v.forwarded_port == Some(51413)
                    && v.egress_matches),
            "the tunnel's exit, country, forwarded port, and a matching egress"
        );
    }

    #[tokio::test]
    async fn a_client_whose_egress_differs_from_the_tunnel_is_flagged() {
        let tunnel = Tunnel {
            client_ip: Some("198.51.100.9"),
            ..healthy_tunnel()
        };
        let ctx = vpn_ctx(
            tunnel_engine(true, Some(tunnel)),
            Some("https://echo"),
            forwarding(),
            Protocols::both(),
        );
        assert!(matches!(vpn_panel(&ctx).await, Some(Panel::Ready(v)) if !v.egress_matches));
    }

    #[tokio::test]
    async fn a_tunnel_that_is_not_running_leaves_the_panel_unavailable() {
        let ctx = vpn_ctx(
            tunnel_engine(false, Some(healthy_tunnel())),
            Some("https://echo"),
            forwarding(),
            Protocols::both(),
        );
        assert!(matches!(
            vpn_panel(&ctx).await,
            Some(Panel::Unavailable { .. })
        ));
    }

    #[tokio::test]
    async fn a_tunnel_that_returns_no_address_is_unavailable() {
        let tunnel = Tunnel {
            gateway_ip: None,
            ..healthy_tunnel()
        };
        let ctx = vpn_ctx(
            tunnel_engine(true, Some(tunnel)),
            Some("https://echo"),
            forwarding(),
            Protocols::both(),
        );
        assert!(matches!(
            vpn_panel(&ctx).await,
            Some(Panel::Unavailable { .. })
        ));
    }

    #[tokio::test]
    async fn a_tunnel_the_engine_cannot_exec_is_unavailable() {
        // The gateway is up but the engine has nothing scripted, so the exec fails.
        let ctx = vpn_ctx(
            tunnel_engine(true, None),
            Some("https://echo"),
            forwarding(),
            Protocols::both(),
        );
        assert!(matches!(
            vpn_panel(&ctx).await,
            Some(Panel::Unavailable { .. })
        ));
    }

    #[tokio::test]
    async fn an_unreachable_engine_leaves_the_vpn_panel_unavailable() {
        let ctx = vpn_ctx(
            Reporting::absent(),
            Some("https://echo"),
            forwarding(),
            Protocols::both(),
        );
        assert!(matches!(
            vpn_panel(&ctx).await,
            Some(Panel::Unavailable { .. })
        ));
    }

    #[tokio::test]
    async fn leak_detection_switched_off_leaves_the_egress_unreadable() {
        let ctx = vpn_ctx(
            tunnel_engine(true, Some(healthy_tunnel())),
            None,
            forwarding(),
            Protocols::both(),
        );
        assert!(matches!(
            vpn_panel(&ctx).await,
            Some(Panel::Unavailable { .. })
        ));
    }

    #[tokio::test]
    async fn a_stack_without_a_torrent_client_has_no_vpn_panel() {
        let ctx = vpn_ctx(
            tunnel_engine(true, Some(healthy_tunnel())),
            Some("https://echo"),
            forwarding(),
            Protocols {
                torrent: false,
                usenet: true,
            },
        );
        assert!(vpn_panel(&ctx).await.is_none());
    }

    #[tokio::test]
    async fn port_forwarding_off_reads_no_forwarded_port() {
        let ctx = vpn_ctx(
            tunnel_engine(true, Some(healthy_tunnel())),
            Some("https://echo"),
            PortForward::default(),
            Protocols::both(),
        );
        assert!(
            matches!(vpn_panel(&ctx).await, Some(Panel::Ready(v)) if v.forwarded_port.is_none())
        );
    }

    #[tokio::test]
    async fn a_provider_that_granted_no_port_reads_none() {
        let tunnel = Tunnel {
            port: None,
            ..healthy_tunnel()
        };
        let ctx = vpn_ctx(
            tunnel_engine(true, Some(tunnel)),
            Some("https://echo"),
            forwarding(),
            Protocols::both(),
        );
        assert!(
            matches!(vpn_panel(&ctx).await, Some(Panel::Ready(v)) if v.forwarded_port.is_none())
        );
    }

    #[tokio::test]
    async fn a_tunnel_without_a_country_still_reads() {
        let tunnel = Tunnel {
            country: None,
            ..healthy_tunnel()
        };
        let ctx = vpn_ctx(
            tunnel_engine(true, Some(tunnel)),
            Some("https://echo"),
            forwarding(),
            Protocols::both(),
        );
        assert!(matches!(vpn_panel(&ctx).await, Some(Panel::Ready(v)) if v.country.is_empty()));
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_leaves_the_vpn_panel_unavailable() {
        // An unreadable stack: the manifest resolves to a reason, and the driver
        // carries it into the panel rather than omitting it.
        let settings = Settings {
            protocols: Protocols::both(),
            data_root: Some(PathBuf::from("/srv/media")),
            ip_echo: Some("https://echo".to_owned()),
            port_forward: forwarding(),
            ..Settings::default()
        };
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(tunnel_engine(true, Some(healthy_tunnel()))),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            Source::External(std::path::Path::new("/lemonfiber/no/such/stack")),
            settings,
            Environment::MacOs,
        );
        assert!(matches!(
            vpn_panel(&ctx).await,
            Some(Panel::Unavailable { .. })
        ));
    }

    #[tokio::test]
    async fn a_torrent_stack_with_no_vpn_pair_does_not_apply() {
        use crate::doctor::vpn::{read_vpn, VpnReading};
        let manifest = lemonfiber_manifest::Manifest {
            schema_version: 1,
            stack_version: String::new(),
            min_cli_version: String::new(),
            profiles: Vec::new(),
            forms: Vec::new(),
            services: Vec::new(),
        };
        let reading = read_vpn(
            &Reporting::absent(),
            "lemonfiber",
            &manifest,
            Protocols::both(),
            Some("https://echo"),
            true,
        )
        .await;
        assert!(matches!(reading, VpnReading::NotApplicable));
    }

    // ── Storage: hardlink & exhaustion ────────────────────────────

    /// A transfer moving at the given speed — the only field the rate reads.
    fn a_transfer(speed: Reading<u64>) -> Transfer {
        Transfer {
            name: "download".to_owned(),
            protocol: Protocol::Torrent,
            progress: 0,
            speed,
            eta: None,
        }
    }

    #[test]
    fn the_download_rate_sums_the_speeds_actually_reported() {
        let transfers = Panel::Ready(vec![
            a_transfer(Reading::Known(1000)),
            a_transfer(Reading::Known(500)),
            // A source that went quiet contributes nothing rather than a guess.
            a_transfer(Reading::Unknown),
        ]);
        assert_eq!(super::download_rate(&transfers), 1500);
        // An unavailable panel has no rate to project exhaustion against.
        let down: Panel<Vec<Transfer>> = Panel::unavailable("down");
        assert_eq!(super::download_rate(&down), 0);
    }

    #[test]
    fn the_hardlink_status_reflects_the_empirical_probe() {
        use crate::storage::Linked;
        assert_eq!(
            super::hardlink_of(&Linked::Yes { links: 2 }),
            Hardlink::Linking
        );
        assert_eq!(super::hardlink_of(&Linked::No), Hardlink::Copying);
        // An unwritable location or an unconfirmed link is never a met guarantee.
        assert_eq!(
            super::hardlink_of(&Linked::Unwritable {
                message: "read-only".to_owned()
            }),
            Hardlink::Unknown
        );
        assert_eq!(super::hardlink_of(&Linked::Unconfirmed), Hardlink::Unknown);
    }

    #[tokio::test]
    async fn storage_projects_exhaustion_from_the_download_rate() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
        let ctx = ctx(engine).with_filesystem(Arc::new(
            SeedFs::keyed(None, None).with_facts(facts(3600, 10000)),
        ));
        // 3600 bytes free, draining at 60 B/s, is a minute until full.
        assert!(matches!(
            super::storage(&ctx, 60).await,
            Panel::Ready(s) if s.exhaustion == Some(Duration::from_secs(60))
        ));
    }

    #[tokio::test]
    async fn storage_projects_no_exhaustion_when_nothing_is_draining() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
        let ctx = ctx(engine).with_filesystem(Arc::new(
            SeedFs::keyed(None, None).with_facts(facts(3600, 10000)),
        ));
        // A rate of zero divides to no estimate rather than an infinite one.
        assert!(matches!(
            super::storage(&ctx, 0).await,
            Panel::Ready(s) if s.exhaustion.is_none()
        ));
    }

    // ── What the tunnel panel means to the summary ────────────────

    /// A filled VPN panel whose client egress does or does not match the tunnel's.
    fn vpn_panel_with(egress_matches: bool) -> Panel<Vpn> {
        Panel::Ready(Vpn {
            exit_ip: "203.0.113.7".to_owned(),
            country: "NL".to_owned(),
            forwarded_port: None,
            egress_matches,
        })
    }

    #[test]
    fn the_tunnel_panel_reads_across_to_the_summary_without_losing_the_middle_case() {
        use crate::health::Egress;
        // No panel is no VPN to be wrong about; a panel that could not be filled is
        // unverified rather than fine — the distinction the summary rests on.
        assert_eq!(super::egress(None), Egress::NotApplicable);
        assert_eq!(
            super::egress(Some(&Panel::unavailable("the gateway did not answer"))),
            Egress::Unreadable
        );
        assert_eq!(super::egress(Some(&vpn_panel_with(true))), Egress::Behind);
        assert_eq!(super::egress(Some(&vpn_panel_with(false))), Egress::Leaking);
    }

    #[tokio::test]
    async fn a_healthy_stack_leaking_outside_the_tunnel_is_summarised_as_critical() {
        // The case the summary exists for, end to end: every container the engine
        // reports is healthy, and the download client's traffic is not behind the
        // tunnel. A count of running containers would call this fine.
        let tunnel = Tunnel {
            client_ip: Some("198.51.100.9"),
            ..healthy_tunnel()
        };
        let ctx = vpn_ctx(
            tunnel_engine(true, Some(tunnel)),
            Some("https://echo"),
            forwarding(),
            Protocols::both(),
        );
        let snapshot = gather(&ctx).await;
        assert_eq!(snapshot.health.standing, Standing::Critical);
        assert!(snapshot.health.standing.wants_attention());
        // And the screen itself is fine, which is a separate matter entirely.
        assert_eq!(snapshot.telemetry, Telemetry::Live);
    }

    #[tokio::test]
    async fn a_stack_behind_its_tunnel_with_nothing_wrong_is_healthy() {
        let ctx = vpn_ctx(
            tunnel_engine(true, Some(healthy_tunnel())),
            Some("https://echo"),
            forwarding(),
            Protocols::both(),
        );
        let snapshot = gather(&ctx).await;
        assert_eq!(snapshot.health.standing, Standing::Healthy);
        assert_eq!(snapshot.health.said(), "healthy");
    }
}
