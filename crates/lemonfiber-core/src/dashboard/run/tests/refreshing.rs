//! What one refresh carries over from the last, and what it says.

use super::*;

#[test]
fn the_download_rate_sums_the_speeds_actually_reported() {
    let transfers = Panel::Ready(vec![
        a_transfer(Reading::Known(1000)),
        a_transfer(Reading::Known(500)),
        // A source that went quiet contributes nothing rather than a guess.
        a_transfer(Reading::Unknown),
    ]);
    assert_eq!(super::super::download_rate(&transfers), 1500);
    // An unavailable panel has no rate to project exhaustion against.
    let down: Panel<Vec<Transfer>> = Panel::unavailable("down");
    assert_eq!(super::super::download_rate(&down), 0);
}

#[test]
fn the_hardlink_status_reflects_the_empirical_probe() {
    use crate::storage::Linked;
    assert_eq!(
        super::super::hardlink_of(&Linked::Yes { links: 2 }),
        Hardlink::Linking
    );
    assert_eq!(super::super::hardlink_of(&Linked::No), Hardlink::Copying);
    // An unwritable location or an unconfirmed link is never a met guarantee.
    assert_eq!(
        super::super::hardlink_of(&Linked::Unwritable {
            message: "read-only".to_owned()
        }),
        Hardlink::Unknown
    );
    assert_eq!(
        super::super::hardlink_of(&Linked::Unconfirmed),
        Hardlink::Unknown
    );
}

#[tokio::test]
async fn storage_projects_exhaustion_from_the_download_rate() {
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
    let ctx = ctx(engine).with_filesystem(Arc::new(
        SeedFs::keyed(None, None).with_facts(facts(3600, 10000)),
    ));
    // 3600 bytes free, draining at 60 B/s, is a minute until full.
    assert!(matches!(
        super::super::storage(&ctx, 60, None).await,
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
        super::super::storage(&ctx, 0, None).await,
        Panel::Ready(s) if s.exhaustion.is_none()
    ));
}

#[tokio::test]
async fn a_volume_that_read_last_refresh_and_not_this_one_reads_stale() {
    // End to end, because the middle state was reachable in the type and not in
    // the assembler: every `Reading` it built was known or unknown, so the
    // distinction the whole model rests on had a state nothing ever entered.
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
    let reading = ctx(engine).with_filesystem(Arc::new(
        SeedFs::keyed(None, None).with_facts(facts(42, 100)),
    ));
    let first = gather(&reading, None).await;
    assert!(matches!(first.storage, Panel::Ready(ref s) if s.free == Reading::Known(42)));

    // The same volume, now unreadable — a zero total is a mount it could not be
    // attributed to, not a full disk.
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
    let quiet =
        ctx(engine).with_filesystem(Arc::new(SeedFs::keyed(None, None).with_facts(facts(0, 0))));
    let second = gather(&quiet, Some(&first)).await;
    assert!(
        matches!(second.storage, Panel::Ready(ref s) if s.free == Reading::Stale(42)),
        "the last figure it gave, marked stale rather than blanked"
    );

    // And with nothing to carry forward it is still unknown, never a zero that
    // would read as a full disk.
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
    let cold =
        ctx(engine).with_filesystem(Arc::new(SeedFs::keyed(None, None).with_facts(facts(0, 0))));
    let alone = gather(&cold, None).await;
    assert!(matches!(alone.storage, Panel::Ready(ref s) if s.free == Reading::Unknown));
}

#[test]
fn a_download_carries_its_last_speed_across_a_refresh_that_did_not_report_one() {
    // Matched by name, since that is what identifies the same download between
    // refreshes; one that has only just appeared has nothing to carry.
    let before = crate::dashboard::Snapshot {
        telemetry: Telemetry::Live,
        health: crate::health::Summary::of(crate::health::Reach::Running, &[], "1000"),
        vpn: None,
        transfers: Panel::Ready(vec![a_transfer(Reading::Known(4096))]),
        queue: Panel::Ready(Vec::new()),
        stuck: Vec::new(),
        alerts: Vec::new(),
        storage: Panel::unavailable("not read here"),
        services: Panel::Ready(Vec::new()),
        door: Panel::unavailable("not read here"),
        household: Panel::unavailable("not read here"),
    };
    assert_eq!(
        super::super::last_speed(Some(&before), "download"),
        Some(&Reading::Known(4096))
    );
    assert_eq!(
        super::super::last_speed(Some(&before), "something else"),
        None
    );
    assert_eq!(super::super::last_speed(None, "download"), None);
}

/// A context whose records land in an emptied scratch directory, so a refresh
/// can be run twice and the second one read what the first left.
fn ctx_remembering(name: &str, engine: Reporting) -> Ctx {
    let dir =
        std::env::temp_dir().join(format!("lemonfiber-refresh-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let settings = Settings {
        protocols: Protocols::both(),
        data_root: Some(std::path::PathBuf::from("/srv/media")),
        env_file: Some(dir.join(".env")),
        ..Settings::default()
    };
    a_context()
        .engine(Arc::new(engine))
        .settings(settings)
        .build()
        .waiting(Duration::ZERO)
}

#[tokio::test]
async fn a_refresh_tells_the_operator_what_it_found() {
    // The whole point of the driver: the health check, the queue check and the
    // notifier all run because a refresh ran, without anybody asking for them —
    // which is the one thing their own tests cannot say about them.
    let ctx = ctx_remembering("tells", Reporting::absent());
    let snapshot = gather(&ctx, None).await;

    // Nothing is running, so there is something to say about it — and the
    // screen is the channel that carries it, needing no configuration.
    assert!(!snapshot.alerts.is_empty(), "{:?}", snapshot.alerts);
}

#[tokio::test]
async fn what_was_said_once_is_not_said_again_on_the_next_refresh() {
    // A screen refreshing once a second must not re-announce a standing fault
    // every second. The outbox is what stops it, and it only stops it because
    // it now survives the refresh that wrote it.
    let ctx = ctx_remembering("once", Reporting::absent());
    let first = gather(&ctx, None).await;
    let again = gather(&ctx, Some(&first)).await;

    assert!(
        !first.alerts.is_empty(),
        "something was said the first time"
    );
    let repeated: Vec<&crate::alert::Alert> = again
        .alerts
        .iter()
        .filter(|alert| !first.alerts.contains(alert))
        .collect();
    assert!(
        repeated.is_empty(),
        "nothing new was invented: {repeated:?}"
    );
}
