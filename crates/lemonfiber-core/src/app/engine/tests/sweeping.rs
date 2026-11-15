//! The networks the engine holds for the project that the project no longer declares,
//! taken away before a start and only named by a rehearsal.

use std::sync::{Arc, Mutex};

use lemonfiber_fixtures::networked::Networked;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_fixtures::support::{spoke, Recording};

use crate::app::{Command, Ctx, Outcome};
use crate::model::LifecycleReport;
use crate::ports::process::Runner;

/// What the project names each network the engine holds: one nothing declares any more,
/// one the project declares, and one the operator's overlay does.
const HELD: [(&str, &str); 3] = [
    ("lemonfiber", "default"),
    ("lemonfiber", "own__prowlarr"),
    ("lemonfiber", "theirs"),
];

/// The engine's name for the network the project no longer declares.
const STALE: &str = "lemonfiber_default";

/// Which networks were taken away by the time Compose was asked to bring anything up.
type AtUp = Arc<Mutex<Option<Vec<String>>>>;

/// A runner noting which networks were taken away by the time Compose is asked to bring
/// anything up, with where it notes them.
fn watching(networks: &Arc<Networked>) -> (Arc<Recording>, AtUp) {
    let at_up: AtUp = Arc::new(Mutex::new(None));
    let noted = Arc::clone(&at_up);
    let networks = Arc::clone(networks);
    let runner = Recording::answering(Ok(spoke(""))).on("up", move || {
        if let Ok(mut at_up) = noted.lock() {
            *at_up = Some(networks.taken());
        }
    });
    (Arc::new(runner), at_up)
}

/// A machine whose engine holds `HELD`, running the stack through `runner`.
fn machine(name: &str, networks: &Arc<Networked>, runner: Arc<dyn Runner>) -> (Scratch, Ctx) {
    let dir = Scratch::new(&format!("sweeping-{name}"));
    let config = dir.join("config");
    let data = dir.join("data").join("media");
    let _ = std::fs::create_dir_all(&config);
    let _ = std::fs::create_dir_all(&data);
    let env = config.join(".env");
    let _ = std::fs::write(&env, format!("DATA_ROOT={}\n", data.display()));
    let _ = std::fs::write(
        crate::overlay::at(&env),
        "services: {}\nnetworks:\n  theirs: {}\n",
    );
    let ctx = crate::test_support::a_context()
        .runner(runner)
        .engine(Arc::new(lemonfiber_fixtures::support::Reporting::holding(
            &["prowlarr", "flaresolverr", "nzbhydra2"],
            crate::ports::docker::Lifecycle::Running,
            crate::ports::docker::Health::Healthy,
        )))
        .networks(networks.clone())
        .settings(crate::config::Settings {
            env_file: Some(env),
            stack_dir: Some(dir.join("data").join("stack")),
            data_root: Some(data),
            ..crate::config::Settings::default()
        })
        .build()
        .with_patience(std::time::Duration::ZERO);
    (dir, ctx)
}

/// What starting the indexer's form came to.
async fn started(ctx: &Ctx) -> Option<LifecycleReport> {
    match crate::app::dispatch(
        Command::Up {
            forms: vec!["search".to_owned()],
            offer: None,
        },
        ctx,
    )
    .await
    {
        Ok(Outcome::Lifecycle(report)) => Some(*report),
        _ => None,
    }
}

#[tokio::test]
async fn a_rehearsed_start_names_the_networks_it_would_take_away_and_takes_none() {
    let networks = Networked::holding(&HELD);
    let (_dir, ctx) = machine(
        "rehearsed",
        &networks,
        Arc::new(lemonfiber_fixtures::support::Scripted(Ok(spoke("")))),
    );

    let report = started(&ctx.rehearsing()).await;

    assert_eq!(
        report.map(|report| report.swept),
        Some(vec![STALE.to_owned()])
    );
    assert!(networks.taken().is_empty());
}

#[tokio::test]
async fn a_start_takes_away_what_the_project_no_longer_declares_before_anything_comes_up() {
    let networks = Networked::holding(&HELD);
    let (runner, at_up) = watching(&networks);
    let (_dir, ctx) = machine("started", &networks, runner);

    let report = started(&ctx).await;

    assert_eq!(
        report.map(|report| report.swept),
        Some(vec![STALE.to_owned()])
    );
    assert_eq!(networks.taken(), [STALE]);
    assert_eq!(
        at_up.lock().ok().and_then(|at_up| at_up.clone()),
        Some(vec![STALE.to_owned()]),
        "taken away before Compose was asked to bring anything up"
    );
}

#[tokio::test]
async fn a_network_that_will_not_go_is_said_and_the_start_goes_on() {
    let networks = Networked::refusing(&HELD);
    let heard = Arc::new(lemonfiber_fixtures::heard::Heard::default());
    let (_dir, ctx) = machine(
        "refused",
        &networks,
        Arc::new(lemonfiber_fixtures::support::Scripted(Ok(spoke("")))),
    );
    let ctx = ctx.with_narrator(heard.clone() as Arc<dyn crate::ports::Narrator>);

    let report = started(&ctx).await;

    assert!(report.is_some_and(|report| report.status == Some(0)));
    assert!(heard
        .said()
        .iter()
        .any(|said| said.contains(STALE) && said.contains("could not be taken away")));
}

#[tokio::test]
async fn a_start_whose_profiles_cannot_be_written_beside_the_project_brings_nothing_up() {
    let networks = Networked::holding(&HELD);
    let (runner, at_up) = watching(&networks);
    let (_dir, ctx) = machine("unprofiled", &networks, runner);
    let project = crate::app::project::directory(&ctx).unwrap_or_default();
    let _ = std::fs::create_dir_all(&project);
    let _ = std::fs::remove_file(project.join(".env"));
    let _ = std::fs::create_dir_all(project.join(".env"));

    assert!(started(&ctx).await.is_none());
    assert!(
        at_up.lock().ok().is_some_and(|at_up| at_up.is_none()),
        "Compose was not asked to bring anything up"
    );
}
