//! A slow source holds up no panel beside it: what answers within the first wave goes
//! out at once, and a second snapshot fills in the rest.

use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;

use super::*;
use crate::dashboard::pace::{FIRST_WAVE, PANEL_WITHIN};
use crate::dashboard::run::{paced, waves};
use crate::dashboard::Snapshot;
use crate::ports::docker::{Container, Engine, ExecOutput, Failure, LogLine, LogQuery, Stats};
use crate::test_support::guard;

/// An engine that lists at once and takes `delay` over every command run inside a
/// container, which is how the tunnel is asked what it is doing.
struct SlowInside {
    /// The engine that answers once the delay is over.
    engine: Reporting,
    /// How long each command inside a container takes.
    delay: Duration,
}

#[async_trait]
impl Engine for SlowInside {
    async fn list(&self, project: &str) -> Result<Vec<Container>, Failure> {
        self.engine.list(project).await
    }

    async fn exec(&self, container: &str, argv: &[String]) -> Result<ExecOutput, Failure> {
        tokio::time::sleep(self.delay).await;
        self.engine.exec(container, argv).await
    }

    async fn stats(&self, project: &str) -> Result<Receiver<(String, Stats)>, Failure> {
        self.engine.stats(project).await
    }

    async fn logs(
        &self,
        project: &str,
        services: &[String],
        query: LogQuery,
    ) -> Result<Receiver<LogLine>, Failure> {
        self.engine.logs(project, services, query).await
    }
}

/// A stack behind a healthy tunnel, its tunnel taking `delay` to answer each question.
fn tunnel_taking(name: &str, delay: Duration) -> Ctx {
    through(name, healthy_tunnel(), delay)
}

/// A stack behind `tunnel`, which takes `delay` to answer each question.
fn through(name: &str, tunnel: Tunnel, delay: Duration) -> Ctx {
    let env = env_without_password(name);
    let settings = Settings {
        protocols: Protocols::both(),
        data_root: Some(PathBuf::from("/srv/media")),
        ip_echo: vec!["https://echo".to_owned()],
        port_forward: forwarding(),
        stack_dir: env.parent().map(|config| config.join("data").join("stack")),
        env_file: Some(env),
        ..Settings::default()
    };
    let engine = tunnel_engine(true, Some(tunnel)).alongside(guard::running());
    guard::keyed(
        a_context()
            .engine(Arc::new(SlowInside { engine, delay }))
            .settings(settings)
            .build()
            .with_patience(Duration::ZERO)
            .with_http(Fake::by_path(vec![guard::forwarding(Some(51413))])),
    )
}

/// Every first wave a gather handed over.
fn handed() -> (
    Arc<std::sync::Mutex<Vec<Snapshot>>>,
    impl FnOnce(Snapshot) + Send,
) {
    let firsts = Arc::new(std::sync::Mutex::new(Vec::new()));
    let kept = Arc::clone(&firsts);
    (firsts, move |first| {
        if let Ok(mut firsts) = kept.lock() {
            firsts.push(first);
        }
    })
}

/// A tunnel slower than the first wave leaves its panel pending and the summary
/// unknown in a first snapshot that the services beside it fill, and the second
/// snapshot fills the tunnel in and says healthy.
#[tokio::test(start_paused = true)]
async fn a_slow_tunnel_is_pending_in_the_first_wave_and_the_summary_is_never_healthy() {
    let ctx = tunnel_taking("waves-slow-tunnel", FIRST_WAVE * 3);
    let (firsts, first) = handed();

    let began = tokio::time::Instant::now();
    let second = waves(&ctx, None, first).await;

    let firsts = firsts
        .lock()
        .map(|firsts| firsts.clone())
        .unwrap_or_default();
    let [early] = firsts.as_slice() else {
        panic!("one first wave, where the tunnel was slower than it: {firsts:?}");
    };
    assert_eq!(early.vpn, Some(Panel::Pending));
    assert_eq!(early.health.standing, Standing::Unknown);
    assert!(early.services.is_available(), "{:?}", early.services);
    assert_eq!(early.telemetry, Telemetry::Live);

    assert!(
        began.elapsed() < PANEL_WITHIN,
        "the tunnel answered within its bound"
    );
    assert!(matches!(second.snapshot.vpn, Some(Panel::Ready(_))));
    assert_eq!(second.snapshot.health.standing, Standing::Healthy);
}

/// Where every source answers within the first wave there is one snapshot, and no
/// first wave is handed over.
#[tokio::test(start_paused = true)]
async fn a_gather_whose_sources_all_answer_at_once_hands_over_one_snapshot() {
    let ctx = tunnel_taking("waves-at-once", Duration::ZERO);
    let (firsts, first) = handed();

    let gathered = waves(&ctx, None, first).await;

    assert!(firsts.lock().is_ok_and(|firsts| firsts.is_empty()));
    assert_eq!(gathered.snapshot.health.standing, Standing::Healthy);
}

/// A one-shot read waits for both waves: nothing in what it answers is pending.
#[tokio::test(start_paused = true)]
async fn a_one_shot_read_waits_for_both_waves() {
    let ctx = tunnel_taking("waves-one-shot", FIRST_WAVE * 3);

    let snapshot = gather(&ctx, None).await;

    assert!(matches!(snapshot.vpn, Some(Panel::Ready(_))));
    assert_eq!(snapshot.health.standing, Standing::Healthy);
    let paced = paced(&ctx, None).await;
    assert!(matches!(paced.snapshot.vpn, Some(Panel::Ready(_))));
}

/// A panel still being read where the last gather filled it shows what it showed
/// then rather than turning pending, so a source slower than the first wave on every
/// refresh does not blank its panel once a second.
#[tokio::test(start_paused = true)]
async fn a_panel_the_last_gather_filled_is_carried_rather_than_pending() {
    let ctx = tunnel_taking("waves-carried", FIRST_WAVE * 3);
    let filled = paced(&ctx, None).await;
    let (firsts, first) = handed();

    let _second = waves(&ctx, Some(&filled.due_now()), first).await;

    let firsts = firsts
        .lock()
        .map(|firsts| firsts.clone())
        .unwrap_or_default();
    let [early] = firsts.as_slice() else {
        panic!("one first wave: {firsts:?}");
    };
    assert_eq!(early.vpn, filled.snapshot.vpn);
    assert_eq!(early.health.standing, Standing::Healthy);
}

/// A first wave records nothing: what it found is a picture, and only the second
/// wave writes what the stack saw down and tells the operator.
#[tokio::test(start_paused = true)]
async fn a_first_wave_writes_nothing_down() {
    let leaking = Tunnel {
        client_ip: Some("198.51.100.9"),
        ..healthy_tunnel()
    };
    let ctx = through("waves-unrecorded", leaking, FIRST_WAVE * 3);
    let history = ctx
        .settings
        .env_file
        .as_deref()
        .and_then(std::path::Path::parent)
        .map(|config| config.join("conditions.json"));
    let (firsts, first) = handed();
    let seen = Arc::new(std::sync::Mutex::new(None));
    let looked = Arc::clone(&seen);
    let looking = history.clone();

    let second = waves(&ctx, None, move |snapshot| {
        if let Ok(mut seen) = looked.lock() {
            *seen = Some(looking.as_deref().is_some_and(std::path::Path::exists));
        }
        first(snapshot);
    })
    .await;

    let firsts = firsts
        .lock()
        .map(|firsts| firsts.clone())
        .unwrap_or_default();
    let [early] = firsts.as_slice() else {
        panic!("one first wave: {firsts:?}");
    };
    assert_eq!(early.health.standing, Standing::Unknown);
    assert!(early.alerts.is_empty(), "{:?}", early.alerts);
    assert_eq!(seen.lock().ok().and_then(|seen| *seen), Some(false));
    assert_eq!(second.snapshot.health.standing, Standing::Critical);
    assert!(history.as_deref().is_some_and(std::path::Path::exists));
}
