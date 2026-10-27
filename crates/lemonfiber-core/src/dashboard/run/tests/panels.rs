//! The stack, door, queue, storage and transfers panels.

use super::*;

/// A queue as a curator reports it: four items, one of them stuck.
fn four_queued() -> Answer {
    let queued = |status: &str| crate::ports::service::Queued {
        title: "something".to_owned(),
        status: status.to_owned(),
        state: "downloading".to_owned(),
        message: None,
        download_id: None,
        grabs: 1,
    };
    crate::test_support::curators::answered(&crate::ports::service::Queue {
        total: 4,
        items: vec![queued("warning"), queued("ok")],
    })
}

#[tokio::test]
async fn a_running_stack_fills_the_services_and_health_and_reads_as_up() {
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
    let snapshot = gather(&ctx(engine), None).await;

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
    let snapshot = gather(&ctx(engine), None).await;
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
    let snapshot = gather(&ctx(engine), None).await;
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
    let ctx = a_context()
        .engine(Arc::new(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            Health::Healthy,
        )))
        .over(nowhere)
        .settings(settings)
        .build();
    let snapshot = gather(&ctx, None).await;
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

/// The address a gathered door came out with, at whichever step it is missing.
fn addressed(door: &Panel<crate::model::FrontDoorReport>) -> Option<String> {
    filled(door)
        .and_then(|door| door.address.as_ref())
        .map(|address| address.url.clone())
}

#[tokio::test]
async fn the_screen_gathers_the_address_to_hand_the_household() {
    // On the screen without being asked, because the operator who needs it has
    // just been asked what to open by somebody in the next room. Built from the
    // same reading the panels beside it are, so the screen and the question
    // cannot name different doors.
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
    let watching = crate::test_support::parts_recorded(
        ctx(engine).with_site(Renamed::called(Some("kitchen-nas"))),
        "panel-door",
    );
    let door = gather(&watching, None).await.door;

    assert_eq!(
        filled(&door).and_then(|door| door.service.clone()),
        Some("Seerr".to_owned())
    );
    assert_eq!(
        addressed(&door),
        Some("http://kitchen-nas.local:5055".to_owned())
    );
}

#[tokio::test]
async fn a_machine_renamed_between_refreshes_is_addressed_as_it_is_now() {
    // Nothing is remembered, so the second gather is the second answer rather
    // than the first — which is the whole of how a changed address is noticed on
    // a screen that is open all day.
    let renamed = Renamed::called(Some("kitchen-nas")).then(Some("cupboard-nas"));
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
    let watching = crate::test_support::parts_recorded(
        ctx(engine).with_site(Arc::clone(&renamed) as Arc<dyn crate::ports::Site>),
        "panel-renamed",
    );

    let first = addressed(&gather(&watching, None).await.door);
    let again = addressed(&gather(&watching, None).await.door);

    assert_eq!(first, Some("http://kitchen-nas.local:5055".to_owned()));
    assert_eq!(again, Some("http://cupboard-nas.local:5055".to_owned()));
}

#[tokio::test]
async fn an_unreadable_register_leaves_the_door_panel_on_the_stacks_own_door() {
    // As the command that shows the door does: what is installed failing to read
    // narrows the candidates to the stack's own, and the panel still names a door.
    let env = crate::test_support::env_without_password("panel-unread");
    let beside = env.with_file_name(crate::config::paths::PLUGINS);
    assert!(std::fs::write(&beside, "not a register").is_ok());
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
    let watching = a_context()
        .engine(Arc::new(engine))
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_site(Renamed::called(Some("kitchen-nas")));
    let door = gather(&watching, None).await.door;

    assert_eq!(
        filled(&door).and_then(|door| door.service.clone()),
        Some("Jellyfin".to_owned())
    );
}

#[tokio::test]
async fn a_stack_that_cannot_be_read_leaves_the_door_panel_saying_why() {
    let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
    let unreadable = a_context().engine(Arc::new(engine)).over(nowhere).build();
    let door = gather(&unreadable, None).await.door;
    assert_eq!(filled(&door), None, "{door:?}");
}

#[tokio::test]
async fn an_engine_that_will_not_answer_leaves_the_door_panel_saying_why() {
    // The other source a door panel has, and it is the services panel's own
    // reason rather than a second account of the same silence.
    let snapshot = gather(&ctx(Reporting::absent()), None).await;
    assert_eq!(addressed(&snapshot.door), None, "{:?}", snapshot.door);
    assert!(!snapshot.services.is_available());
}

/// A context over the stack this repo carries, every curator moved into its plugin and
/// answering its queue with `queue`, its adapter running where `running` and keyed where
/// `keyed`.
fn curating(queue: Answer, running: bool, keyed: bool) -> Ctx {
    use crate::test_support::adapted;
    use crate::test_support::curators::ALL;
    static BUILT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nth = BUILT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        lemonfiber_fixtures::scratch::Scratch::named(&format!("dashboard-queue-{nth}")).kept();
    let _ = std::fs::create_dir_all(&dir);
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
    let mut ctx = ctx(if running {
        engine.alongside(adapted::running(&ALL))
    } else {
        engine
    })
    .with_http(Fake::by_path(vec![(
        crate::test_support::curators::curated("queue"),
        queue,
    )]));
    ctx.settings.env_file = Some(dir.join(".env"));
    for curator in ALL {
        curator.recorded(&ctx);
    }
    if keyed {
        let adapters: Vec<&str> = ALL.iter().map(|curator| curator.adapter).collect();
        ctx.seams.confined = adapted::Keyed::holding(Arc::clone(&ctx.seams.confined), &adapters);
    }
    ctx
}

#[tokio::test]
async fn the_queue_panel_fills_with_each_curators_depth_and_stuck_count() {
    let snapshot = gather(&curating(four_queued(), true, true), None).await;
    assert!(
        matches!(snapshot.queue, Panel::Ready(ref queues)
            if !queues.is_empty() && queues.iter().all(|q| q.depth == 4 && q.stuck == 1)),
        "each curator that answered contributes its depth and stuck count"
    );
}

/// No adapter holds a key yet: the ordinary first-start case, skipped so the panel is
/// ready-but-empty rather than failed.
#[tokio::test]
async fn a_curator_still_starting_with_no_key_is_left_out_of_the_queue() {
    let snapshot = gather(&curating(four_queued(), true, false), None).await;
    assert!(matches!(snapshot.queue, Panel::Ready(ref queues) if queues.is_empty()));
}

#[tokio::test]
async fn a_curator_whose_adapter_is_not_running_is_left_out_of_the_queue() {
    let snapshot = gather(&curating(four_queued(), false, true), None).await;
    assert!(matches!(snapshot.queue, Panel::Ready(ref queues) if queues.is_empty()));
}

/// The adapter is asked, but the queue is refused, so that curator is dropped from the
/// panel rather than failing it.
#[tokio::test]
async fn a_curator_that_will_not_answer_its_queue_is_left_out() {
    let snapshot = gather(
        &curating(crate::test_support::curators::refusing(), true, true),
        None,
    )
    .await;
    assert!(matches!(snapshot.queue, Panel::Ready(ref queues) if queues.is_empty()));
}

#[tokio::test]
async fn an_unreachable_engine_leaves_services_unavailable_and_reads_disconnected() {
    let snapshot = gather(&ctx(Reporting::absent()), None).await;
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
    let ctx = a_context()
        .engine(Arc::new(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(settings)
        .build();
    let snapshot = gather(&ctx, None).await;
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
    let ctx = ctx(engine).with_filesystem(Arc::new(SeedFs::keyed(None).with_facts(facts(42, 100))));
    let snapshot = gather(&ctx, None).await;
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
    let ctx = ctx(engine).with_filesystem(Arc::new(SeedFs::keyed(None).with_facts(facts(0, 0))));
    let snapshot = gather(&ctx, None).await;
    assert!(
        matches!(snapshot.storage, Panel::Ready(storage) if storage.free == Reading::Unknown),
        "a volume that could not be read reports unknown free space, not zero"
    );
}

/// A context configured to read download clients: the library stack running beside both
/// download clients' adapters, the given filesystem and transport, and the env file it
/// says, beside which each client is recorded as moved into its plugin.
fn ctx_downloads(fs: SeedFs, http: Arc<dyn Http>, env_file: Option<PathBuf>) -> Ctx {
    let settings = Settings {
        protocols: Protocols::both(),
        data_root: Some(PathBuf::from("/srv/media")),
        env_file,
        ..Settings::default()
    };
    a_context()
        .engine(Arc::new(
            Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
                .alongside(usenet::running())
                .alongside(torrent::running()),
        ))
        .settings(settings)
        .build()
        .with_patience(Duration::ZERO)
        .with_filesystem(Arc::new(fs))
        .with_http(http)
}

#[tokio::test]
async fn the_transfers_panel_fills_from_each_download_client() {
    let http: Arc<dyn Http> =
        usenet::beside_torrents(&[torrent::carrying()], &[usenet::carrying()]);
    let ctx = ctx_downloads(
        SeedFs::keyed(None),
        http,
        Some(env_without_password("transfers-fill")),
    );
    usenet::moved_in(&ctx);
    torrent::moved_in(&ctx);
    let snapshot = gather(&ctx, None).await;

    assert!(
        matches!(&snapshot.transfers, Panel::Ready(active) if active.iter().any(|t|
            matches!(t.protocol, Protocol::Torrent)
                && t.name == torrent::CARRIED
                && t.progress == 30
                && matches!(t.speed, Reading::Known(4096))
                && t.eta == Some(Duration::from_secs(120)))),
        "the torrent client's download fills a torrent transfer, tagged by which client \
         answered rather than by anything the client said"
    );
    assert!(
        matches!(&snapshot.transfers, Panel::Ready(active) if active.iter().any(|t|
            matches!(t.protocol, Protocol::Usenet)
                && t.name == usenet::CARRIED
                && t.progress == 20
                && matches!(t.speed, Reading::Unknown)
                && t.eta == Some(Duration::from_secs(300)))),
        "the Usenet client's download fills a Usenet transfer, a speed it could not read \
         unknown rather than a confident zero that would read as a stall"
    );
    assert!(matches!(&snapshot.transfers, Panel::Ready(active) if active.len() == 2));
}

/// Neither download client recorded as moved in: each is the stack's own, which nothing
/// reaches, so the panel is ready-but-empty rather than failed.
#[tokio::test]
async fn a_client_not_yet_seeded_is_left_out_not_a_failure() {
    let http: Arc<dyn Http> = Fake::silent();
    let ctx = ctx_downloads(SeedFs::keyed(None), http, None);
    let snapshot = gather(&ctx, None).await;
    assert!(matches!(&snapshot.transfers, Panel::Ready(active) if active.is_empty()));
}

/// Both download clients have moved in but neither adapter holds a key yet. Neither can
/// be read, so neither appears, and neither is asked.
#[tokio::test]
async fn a_client_whose_key_is_not_on_disk_yet_is_left_out() {
    let http = Fake::silent();
    let ctx = ctx_downloads(
        SeedFs::keyed(None),
        Arc::clone(&http) as Arc<dyn Http>,
        Some(env_without_password("transfers-unkeyed")),
    );
    usenet::unkeyed(&ctx);
    torrent::unkeyed(&ctx);
    let snapshot = gather(&ctx, None).await;
    assert!(matches!(&snapshot.transfers, Panel::Ready(active) if active.is_empty()));
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// Both download clients have moved in, but neither adapter answers, so both are dropped
/// from the panel rather than failing it.
#[tokio::test]
async fn a_download_client_that_will_not_answer_is_left_out() {
    let http: Arc<dyn Http> = Fake::silent();
    let ctx = ctx_downloads(
        SeedFs::keyed(None),
        http,
        Some(env_without_password("transfers-silent")),
    );
    usenet::moved_in(&ctx);
    torrent::moved_in(&ctx);
    let snapshot = gather(&ctx, None).await;
    assert!(matches!(&snapshot.transfers, Panel::Ready(active) if active.is_empty()));
}
