//! How often each panel is read, and what a panel that will not answer does to the rest.

use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;

use super::*;
use crate::dashboard::pace::{PANEL_WITHIN, VPN_EVERY};
use crate::dashboard::run::paced;
use crate::ports::docker::{Container, Engine, ExecOutput, Failure, LogLine, LogQuery, Stats};

/// A context reading the VPN pair through `engine`, its clock stopped `seconds` into
/// the day the real manifest is checked on.
fn at(seconds: u64, engine: Reporting) -> Ctx {
    let settings = Settings {
        protocols: Protocols::both(),
        data_root: Some(PathBuf::from("/srv/media")),
        ip_echo: vec!["https://echo".to_owned()],
        port_forward: forwarding(),
        ..Settings::default()
    };
    a_context()
        .engine(Arc::new(engine))
        .settings(settings)
        .clock(lemonfiber_fixtures::ports::Stopped::into_today(seconds))
        .build()
        .with_patience(Duration::ZERO)
}

/// Whether the VPN panel shows a tunnel.
fn tunnelled(snapshot: &crate::dashboard::Snapshot) -> bool {
    matches!(snapshot.vpn, Some(Panel::Ready(_)))
}

/// The tunnel is asked at its own pace, and carried forward between readings, while
/// the services beside it are read on every refresh.
#[tokio::test]
async fn the_tunnel_is_read_at_its_own_pace_and_the_services_every_refresh() {
    let up = at(1_000, tunnel_engine(true, Some(healthy_tunnel())));
    let first = paced(&up, None).await;
    assert!(
        tunnelled(&first.snapshot),
        "read on the first refresh: {:?}",
        first.snapshot.vpn
    );

    let down = at(1_001, tunnel_engine(false, Some(healthy_tunnel())));
    let second = paced(&down, Some(&first)).await;
    assert!(
        tunnelled(&second.snapshot),
        "a second later the tunnel is not asked again, and its reading stands"
    );
    assert!(
        matches!((&first.snapshot.services, &second.snapshot.services),
            (Panel::Ready(before), Panel::Ready(after)) if before != after),
        "the services are read afresh every refresh"
    );

    let due = at(
        1_000 + VPN_EVERY.as_secs(),
        tunnel_engine(false, Some(healthy_tunnel())),
    );
    let third = paced(&due, Some(&second)).await;
    assert!(
        !tunnelled(&third.snapshot),
        "once its pace has passed it is asked again"
    );
}

/// A refresh somebody asked for reads every panel, whatever its pace.
#[tokio::test]
async fn a_refresh_asked_for_reads_every_panel() {
    let up = at(1_000, tunnel_engine(true, Some(healthy_tunnel())));
    let first = paced(&up, None).await;

    let down = at(1_001, tunnel_engine(false, Some(healthy_tunnel())));
    let asked = paced(&down, Some(&first.due_now())).await;
    assert!(!tunnelled(&asked.snapshot), "the tunnel was asked again");
}

/// An engine that takes every request and never answers any of them.
struct Hanging;

#[async_trait]
impl Engine for Hanging {
    async fn list(&self, _project: &str) -> Result<Vec<Container>, Failure> {
        std::future::pending().await
    }

    async fn exec(&self, _container: &str, _argv: &[String]) -> Result<ExecOutput, Failure> {
        std::future::pending().await
    }

    async fn stats(&self, _project: &str) -> Result<Receiver<(String, Stats)>, Failure> {
        std::future::pending().await
    }

    async fn logs(
        &self,
        _project: &str,
        _services: &[String],
        _query: LogQuery,
    ) -> Result<Receiver<LogLine>, Failure> {
        std::future::pending().await
    }
}

/// A source that will not answer marks its own panel within its bound, and the panels
/// are read together, so the refresh costs the one bound rather than one per panel.
#[tokio::test(start_paused = true)]
async fn a_source_that_will_not_answer_costs_its_bound_once() {
    let settings = Settings {
        protocols: Protocols::both(),
        data_root: Some(PathBuf::from("/srv/media")),
        ip_echo: vec!["https://echo".to_owned()],
        ..Settings::default()
    };
    let ctx = a_context()
        .engine(Arc::new(Hanging))
        .settings(settings)
        .build()
        .with_patience(Duration::ZERO);

    let began = tokio::time::Instant::now();
    let gathered = paced(&ctx, None).await;
    assert_eq!(began.elapsed(), PANEL_WITHIN, "the bound, once");
    let late = format!("did not answer within {} seconds", PANEL_WITHIN.as_secs());
    assert!(
        matches!(&gathered.snapshot.services, Panel::Unavailable { reason }
            if reason == &format!("the container engine {late}")),
        "{:?}",
        gathered.snapshot.services
    );
    assert!(
        matches!(&gathered.snapshot.vpn, Some(Panel::Unavailable { reason })
            if reason == &format!("the VPN {late}")),
        "{:?}",
        gathered.snapshot.vpn
    );
    let quiet = Duration::from_secs(1);
    assert!(tokio::time::timeout(quiet, Hanging.stats("p"))
        .await
        .is_err());
    assert!(
        tokio::time::timeout(quiet, Hanging.logs("p", &[], LogQuery::recent(1)))
            .await
            .is_err()
    );
}

/// Whether imports link is what the probe found now, unknown where it did not answer,
/// and otherwise what it found last — unknown where it found nothing last either.
#[tokio::test]
async fn whether_imports_link_is_carried_from_the_last_reading_that_had_one() {
    use super::super::{carried_link, Asked};
    let unconfigured = super::super::gather(&a_context().build(), None).await;
    assert!(!unconfigured.storage.is_available());
    assert_eq!(
        carried_link(Asked::NotDue, Some(&unconfigured)),
        Hardlink::Unknown
    );
    assert_eq!(carried_link(Asked::Late, None), Hardlink::Unknown);
    assert_eq!(
        carried_link(Asked::Read(Hardlink::Copying), None),
        Hardlink::Copying
    );
}

/// When the file at `path` was last written, where it is there.
fn written(path: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

/// A refresh that changed nothing in the history writes nothing.
///
/// The history is written beside the settings on every refresh that moves it; one
/// rewritten every second with what it already held is a flush to the disk for
/// nothing.
#[tokio::test]
async fn a_refresh_that_changed_nothing_writes_nothing() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("unchanged-history");
    // Stopped services, so the first refresh has a fault to write down.
    let mut ctx = at(1_000, tunnel_engine(false, Some(healthy_tunnel())));
    ctx.settings.env_file = Some(dir.join(".env"));
    let first = paced(&ctx, None).await;
    let history = dir.join("conditions.json");
    let before = written(&history);
    assert!(before.is_some(), "the first refresh wrote what it found");

    let _ = paced(&ctx, Some(&first)).await;

    assert_eq!(
        written(&history),
        before,
        "the second found the same and wrote nothing"
    );
}
