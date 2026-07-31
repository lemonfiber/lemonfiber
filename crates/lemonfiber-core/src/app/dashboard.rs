//! Assembling one dashboard snapshot from what the ports can be reached for.
//!
//! The shape of the screen and the rules that keep it honest are the pure
//! [`crate::dashboard`] module; this is the driver that fills it from the live
//! stack. It never fails — a dashboard degrades rather than errors (a source that
//! cannot be reached marks its own panel and leaves the rest), so there is no
//! error channel through which a dead source could terminate the render loop.
//!
//! This gatherer fills the services and their health, the storage volume's free
//! space, each \*arr's queue, and each download client's active transfers; the VPN
//! panel has no live source wired yet and says so, rather than being shown as empty
//! or as zero. Its telemetry arrives in the slices that follow, each turning one
//! "not gathered yet" into a real panel.

use std::path::Path;

use lemonfiber_manifest::Manifest;

use crate::app::Ctx;
use crate::dashboard::{
    Hardlink, Health, Panel, Protocol, Queue, Reach, Reading, Snapshot, Standing, Storage, Transfer,
};
use crate::docker::{condition, survey, Condition, Service};
use crate::error::Diagnose;
use crate::ports::service::{Download, Queues, Transfers};
use crate::qbittorrent::Qbittorrent;
use crate::sabnzbd::Sabnzbd;

use super::targets::{
    download_targets, project_directory, recorded_qbittorrent_password, servarr_targets,
    DownloadKind, DownloadTarget,
};

/// Gather one snapshot of what the stack is doing right now.
///
/// Reads the services through the engine and summarises their health; a stack that
/// cannot be reached leaves both unavailable and the standing disconnected. The
/// panels without a gatherer are marked pending — they do not drag the standing
/// down, because a panel nobody has wired is not a source that failed.
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

    let observed = observe(ctx, manifest.as_ref()).await;
    let reach = reach(configured, observed.as_ref());

    let (services, health) = match observed {
        Ok(services) => {
            let health = Health::of(&services);
            (Panel::Ready(services), Panel::Ready(health))
        }
        Err(reason) => (
            Panel::unavailable(reason.clone()),
            Panel::unavailable(reason),
        ),
    };

    Snapshot {
        // Only a source that genuinely failed marks the screen degraded; the
        // pending panels below are not-yet-built, not down, so they pass `false`.
        standing: Standing::read(reach, false),
        health,
        vpn: None,
        transfers: transfers(ctx, manifest.as_ref(), project.as_deref()).await,
        queue: queues(ctx, manifest.as_ref(), project.as_deref()).await,
        storage: storage(ctx).await,
        services,
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
        let downloads = read(ctx, target).await;
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

/// One client's active downloads, read on its own shape — nothing where it is not
/// yet seeded or will not answer, so it is left out rather than failing the panel.
async fn read(ctx: &Ctx, target: &DownloadTarget) -> Vec<Download> {
    match &target.kind {
        DownloadKind::Qbittorrent => {
            let Some(password) = recorded_qbittorrent_password(ctx) else {
                return Vec::new();
            };
            Qbittorrent::authenticated(ctx.http.clone(), &target.base, password)
                .transfers()
                .await
                .unwrap_or_default()
        }
        DownloadKind::Sabnzbd { config } => {
            let Some(text) = ctx.filesystem.read(config).await else {
                return Vec::new();
            };
            let Some(key) = crate::sabnzbd::api_key(&text) else {
                return Vec::new();
            };
            Sabnzbd::new(ctx.http.clone(), &target.base, key)
                .transfers()
                .await
                .unwrap_or_default()
        }
    }
}

/// The protocol a client's transfers move over.
fn protocol_of(kind: &DownloadKind) -> Protocol {
    match kind {
        DownloadKind::Qbittorrent => Protocol::Torrent,
        DownloadKind::Sabnzbd { .. } => Protocol::Usenet,
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

/// The storage picture: how much is free on the data volume.
///
/// Free space is read afresh each refresh, which is a cheap read. A volume that
/// could not be attributed to any mount reports a zero total, and its free space
/// is then unknown rather than zero — "cannot read the volume" and "the disk is
/// full" are opposite things to an operator, and must not render alike. The
/// hardlink status and the projected exhaustion have their own telemetry: a
/// per-refresh hardlink probe would write to disk every second, and exhaustion is
/// projected against the queue, so neither is gathered here yet.
async fn storage(ctx: &Ctx) -> Panel<Storage> {
    let Some(root) = ctx.settings.data_root.as_deref() else {
        return Panel::unavailable("no data location is configured");
    };
    let facts = ctx.filesystem.describe(root).await;
    let free = if facts.total == 0 {
        Reading::Unknown
    } else {
        Reading::Known(facts.available)
    };
    Panel::Ready(Storage {
        free,
        exhaustion: None,
        hardlink: Hardlink::Unknown,
    })
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

/// How far the surface reached, from which the standing is read.
fn reach(configured: bool, observed: Result<&Vec<Service>, &String>) -> Reach {
    if !configured {
        return Reach::Unconfigured;
    }
    match observed {
        Err(_) => Reach::Disconnected,
        Ok(services) if condition(services) == Condition::Inactive => Reach::Idle,
        Ok(_) => Reach::Up,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;

    use super::gather;
    use crate::app::Ctx;
    use crate::config::{Protocols, Settings};
    use crate::dashboard::{Panel, Protocol, Reading, Standing};
    use crate::platform::Environment;
    use crate::ports::docker::{Health, Lifecycle};
    use crate::ports::filesystem::{FsKind, StorageFacts};
    use crate::ports::http::{Http, Request, Response, Unreachable};
    use crate::stack::Source;
    use crate::test_support::{
        a_password, spoke, stack, Reporting, Scripted, ScriptedHttp, SeedFs,
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
        // The health summary is the stack's condition, not the dashboard's: the
        // services the engine reports are healthy, so nothing wants attention, even
        // though the rest of the manifest's services are absent.
        assert!(
            matches!(&snapshot.health, Panel::Ready(health) if health.needing_attention == 0),
            "nothing the engine reported wants attention"
        );
        // Standing is the telemetry state, not the stack's: every source that has a
        // gatherer answered and the pending panels are not failures, so it reads
        // live even though the stack is only partly up.
        assert_eq!(snapshot.standing, Standing::Live);
    }

    #[tokio::test]
    async fn the_vpn_panel_without_a_gatherer_yet_is_omitted_rather_than_shown_empty() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
        let snapshot = gather(&ctx(engine)).await;
        assert!(
            snapshot.vpn.is_none(),
            "the VPN panel is omitted until wired"
        );
        // The transfers panel now has a gatherer, so it is filled — empty here,
        // since the real filesystem holds no download-client credentials to read.
        assert!(matches!(snapshot.transfers, Panel::Ready(ref active) if active.is_empty()));
    }

    #[tokio::test]
    async fn an_idle_stack_reads_as_no_stack() {
        // Containers exist but none is running — configured, reachable, nothing up.
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Exited, Health::None);
        let snapshot = gather(&ctx(engine)).await;
        assert_eq!(snapshot.standing, Standing::NoStack);
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
        assert_eq!(snapshot.standing, Standing::Disconnected);
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
        assert_eq!(snapshot.standing, Standing::Disconnected);
        assert!(
            !snapshot.services.is_available() && !snapshot.health.is_available(),
            "a stack that cannot be read leaves its services and health unavailable, not empty"
        );
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
        assert_eq!(snapshot.standing, Standing::Unconfigured);
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
}
