//! Assembling one dashboard snapshot from what the ports can be reached for.
//!
//! The shape of the screen and the rules that keep it honest are the pure
//! [`crate::dashboard`] module; this is the driver that fills it from the live
//! stack. It never fails — a dashboard degrades rather than errors (a source that
//! cannot be reached marks its own panel and leaves the rest), so there is no
//! error channel through which a dead source could terminate the render loop.
//!
//! This first gatherer fills the services and the health read from them; the VPN,
//! transfers, queue and storage panels have no live source wired yet and say so,
//! rather than being shown as empty or as zero. Their telemetry arrives in the
//! slices that follow, each turning one "not gathered yet" into a real panel.

use crate::app::Ctx;
use crate::dashboard::{Hardlink, Health, Panel, Reach, Reading, Snapshot, Standing, Storage};
use crate::docker::{condition, survey, Condition, Service};
use crate::error::Diagnose;

/// What a panel says while its telemetry has no gatherer yet — stated plainly, so
/// a not-yet-wired source reads as absent rather than as nothing happening.
const PENDING: &str = "no live source for this panel yet";

/// Gather one snapshot of what the stack is doing right now.
///
/// Reads the services through the engine and summarises their health; a stack that
/// cannot be reached leaves both unavailable and the standing disconnected. The
/// panels without a gatherer are marked pending — they do not drag the standing
/// down, because a panel nobody has wired is not a source that failed.
pub async fn gather(ctx: &Ctx) -> Snapshot {
    let configured = ctx.settings.data_root.is_some();
    let observed = observe(ctx).await;
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
        transfers: Panel::unavailable(PENDING),
        queue: Panel::unavailable(PENDING),
        storage: storage(ctx).await,
        services,
    }
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
async fn observe(ctx: &Ctx) -> Result<Vec<Service>, String> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem().summary)?;
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
    Ok(survey(&manifest, &profiles, &containers))
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
    use std::sync::Arc;
    use std::time::Duration;

    use super::gather;
    use crate::app::Ctx;
    use crate::config::{Protocols, Settings};
    use crate::dashboard::{Panel, Reading, Standing};
    use crate::platform::Environment;
    use crate::ports::docker::{Health, Lifecycle};
    use crate::ports::filesystem::{FsKind, StorageFacts};
    use crate::stack::Source;
    use crate::test_support::{spoke, stack, Reporting, Scripted, SeedFs};

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
    async fn the_panels_without_a_gatherer_yet_say_so_rather_than_showing_empty() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
        let snapshot = gather(&ctx(engine)).await;
        assert!(
            snapshot.vpn.is_none(),
            "the VPN panel is omitted until wired"
        );
        // Transfers and queue still have no gatherer; each is a different type, so
        // they are checked one by one.
        assert!(!snapshot.transfers.is_available());
        assert!(!snapshot.queue.is_available());
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
}
