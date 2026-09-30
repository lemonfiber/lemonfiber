use std::sync::Arc;

use super::{after, before, load};
use crate::app::Ctx;
use crate::config::Settings;
use crate::ports::docker::{Container, Health, Lifecycle};
use crate::stack::compose::Action;
use crate::test_support::{a_context, Reporting};

/// A context over this engine whose records land in a directory of the test's own.
fn over(name: &str, engine: Reporting) -> Ctx {
    let dir = crate::app::fixtures::scratch(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    a_context()
        .engine(Arc::new(engine))
        .settings(Settings {
            env_file: Some(dir.join(".env")),
            ..Settings::default()
        })
        .build()
}

/// One exited container, by the id the fake engine gives it.
fn exited(service: &str) -> Container {
    Container {
        id: format!("id-{service}"),
        project: "lemonfiber".to_owned(),
        service: service.to_owned(),
        lifecycle: Lifecycle::Exited,
        health: Health::None,
        published: Vec::new(),
        mounts: Vec::new(),
        exit: Some(1),
    }
}

fn named(services: &[&str]) -> Vec<String> {
    services
        .iter()
        .map(|service| (*service).to_owned())
        .collect()
}

/// A stop writes down the containers it stopped, and only those: a service the stop did
/// not address is read by its own exit code whatever state it is in.
#[tokio::test]
async fn a_stop_writes_down_the_containers_it_stopped() {
    let engine =
        Reporting::holding(&["gluetun", "sonarr"], Lifecycle::Exited, Health::None).exiting(1);
    let ctx = over("halted-a-stop-writes-down", engine);
    let gluetun = named(&["gluetun"]);

    after(&ctx, &Action::Stop(gluetun.clone()), &gluetun).await;

    let halted = load(&ctx);
    assert!(halted.holds(&exited("gluetun")));
    assert!(
        !halted.holds(&exited("sonarr")),
        "a container the stop did not address fell over by itself"
    );
}

/// A start lets go of what it addresses before it runs, so a service that falls over on
/// the way back up reads as the failure it is rather than as the stop before it.
#[tokio::test]
async fn a_start_lets_go_of_what_it_addresses() {
    let engine =
        Reporting::holding(&["gluetun", "sonarr"], Lifecycle::Exited, Health::None).exiting(1);
    let ctx = over("halted-a-start-lets-go", engine);
    let both = named(&["gluetun", "sonarr"]);
    after(&ctx, &Action::Stop(Vec::new()), &both).await;

    let sonarr = named(&["sonarr"]);
    before(&ctx, &Action::Start(sonarr.clone()), &sonarr);

    let halted = load(&ctx);
    assert!(!halted.holds(&exited("sonarr")), "started again, so let go");
    assert!(
        halted.holds(&exited("gluetun")),
        "not started, so still stopped"
    );
}

/// Only a stop writes, and a stop lets nothing go: a stop answered by the actions that run
/// nothing, and a second stop, leave what was written as it was.
#[tokio::test]
async fn only_a_stop_is_written_and_a_stop_lets_nothing_go() {
    let engine = Reporting::holding(&["gluetun"], Lifecycle::Exited, Health::None).exiting(1);
    let ctx = over("halted-only-a-stop", engine);
    let gluetun = named(&["gluetun"]);

    after(&ctx, &Action::Restart(gluetun.clone()), &gluetun).await;
    assert!(
        !load(&ctx).holds(&exited("gluetun")),
        "a restart stops nothing"
    );

    after(&ctx, &Action::Stop(gluetun.clone()), &gluetun).await;
    for action in [Action::Stop(gluetun.clone()), Action::Pull, Action::Config] {
        before(&ctx, &action, &gluetun);
        assert!(load(&ctx).holds(&exited("gluetun")), "{action:?}");
    }
    before(&ctx, &Action::Down, &gluetun);
    assert!(
        !load(&ctx).holds(&exited("gluetun")),
        "taken down, so let go"
    );
}

/// A container still running after the stop was not stopped by it, and an engine that
/// will not answer leaves the record as it was rather than guessing.
#[tokio::test]
async fn only_a_container_that_is_down_is_written() {
    let running = Reporting::holding(&["gluetun"], Lifecycle::Running, Health::None);
    let ctx = over("halted-only-down", running);
    let gluetun = named(&["gluetun"]);
    after(&ctx, &Action::Stop(gluetun.clone()), &gluetun).await;
    assert!(!load(&ctx).holds(&exited("gluetun")));

    let ctx = over("halted-unanswered", Reporting::absent());
    after(&ctx, &Action::Stop(gluetun.clone()), &gluetun).await;
    assert_eq!(load(&ctx), crate::docker::Halted::default());
}
