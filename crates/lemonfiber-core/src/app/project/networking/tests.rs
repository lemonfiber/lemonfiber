//! Every network of the project, created through the engine before a start, kept where
//! the engine holds it as the project gives it, created again where it holds it in another
//! shape, every one held undeclared or in another shape taken away before any is created,
//! and only named by a rehearsal.

use std::sync::Arc;

use lemonfiber_fixtures::networked::Networked;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_fixtures::support::{spoke, Scripted};
use lemonfiber_ports::docker::Network;

use crate::app::{Command, Ctx, Outcome};
use crate::error::codes::stack::NETWORK_NOT_MADE;
use crate::model::LifecycleReport;

/// The network the indexer's form declares between the indexer manager and its adapter.
const OWN: &str = "own__prowlarr";

/// A machine whose engine holds what `networks` holds.
fn machine(name: &str, networks: &Arc<Networked>) -> (Scratch, Ctx) {
    machine_running(
        name,
        networks,
        lemonfiber_fixtures::support::Reporting::holding(
            &["prowlarr", "flaresolverr", "nzbhydra2"],
            crate::ports::docker::Lifecycle::Running,
            crate::ports::docker::Health::Healthy,
        ),
    )
}

/// The same machine, its containers running as `engine` says.
fn machine_running(
    name: &str,
    networks: &Arc<Networked>,
    engine: lemonfiber_fixtures::support::Reporting,
) -> (Scratch, Ctx) {
    let dir = Scratch::new(&format!("networking-{name}"));
    let config = dir.join("config");
    let data = dir.join("data").join("media");
    let _ = std::fs::create_dir_all(&config);
    let _ = std::fs::create_dir_all(&data);
    let env = config.join(".env");
    let _ = std::fs::write(&env, format!("DATA_ROOT={}\n", data.display()));
    let ctx = crate::test_support::a_context()
        .runner(Arc::new(Scripted(Ok(spoke("")))))
        .engine(Arc::new(engine))
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
async fn started(ctx: &Ctx) -> Result<LifecycleReport, String> {
    match crate::app::dispatch(
        Command::Up {
            forms: vec!["search".to_owned()],
            offer: None,
        },
        ctx,
    )
    .await
    {
        Ok(Outcome::Lifecycle(report)) => Ok(*report),
        Ok(_) => Err(String::new()),
        Err(problem) => Err(problem.code.to_string()),
    }
}

/// The engine's name for the network `declared`.
fn named(declared: &str) -> String {
    crate::project::document::named(crate::PRODUCT, declared)
}

/// What the project keeps of the networks the engine answered.
fn kept(ctx: &Ctx) -> String {
    crate::app::project::directory(ctx)
        .and_then(|directory| {
            std::fs::read_to_string(directory.join(crate::fence::NETWORKS_HELD)).ok()
        })
        .unwrap_or_default()
}

/// The project as written.
fn written(ctx: &Ctx) -> String {
    crate::app::project::path(ctx)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default()
}

/// The network `declared` as the engine holds it after the start `ctx` ran.
async fn held(ctx: &Ctx, declared: &str) -> Option<Network> {
    ctx.seams
        .networks
        .networks(crate::PRODUCT)
        .await
        .ok()?
        .into_iter()
        .find(|network| network.name == named(declared))
}

#[tokio::test]
async fn a_start_creates_every_network_and_keeps_each_subnet_gateway_and_id() {
    let networks = Networked::none();
    let (_dir, ctx) = machine("created", &networks);

    let report = started(&ctx).await;

    let made = networks.made();
    assert!(made.contains(&named(OWN)), "{made:?}");
    assert_eq!(
        report.map(|report| report.networks_created),
        Ok(made.clone())
    );
    let own = held(&ctx, OWN).await.unwrap_or_default();
    let line = kept(&ctx)
        .lines()
        .find(|line| line.starts_with(&format!("{OWN} ")))
        .map(str::to_owned);
    assert_eq!(
        line,
        Some(format!(
            "{OWN} {} {} {}",
            own.shape.subnet.unwrap_or_default(),
            own.gateway.unwrap_or_default(),
            own.id
        ))
    );
    let project = written(&ctx);
    assert!(
        project.contains(&format!("name: {}", named(OWN))),
        "{project}"
    );
    assert!(project.contains("external: true"), "{project}");
}

#[tokio::test]
async fn a_network_held_as_the_project_gives_it_is_kept_whichever_program_created_it() {
    let networks = Networked::none();
    let (_dir, ctx) = machine("kept", &networks);
    assert!(started(&ctx).await.is_ok());
    let first = networks.made();

    let again = started(&ctx).await;

    assert_eq!(networks.made(), first, "nothing created a second time");
    assert!(networks.taken().is_empty(), "nothing taken away");
    assert!(again.is_ok_and(
        |report| report.networks_created.is_empty() && report.networks_recreated.is_empty()
    ));
}

#[tokio::test]
async fn a_network_held_in_another_shape_is_created_again_with_a_new_id() {
    let networks = Networked::holding(&[(crate::PRODUCT, OWN)]);
    let (_dir, ctx) = machine("recreated", &networks);

    let report = started(&ctx).await;

    assert_eq!(networks.taken(), [named(OWN)]);
    assert_eq!(
        report.map(|report| report.networks_recreated),
        Ok(vec![named(OWN)])
    );
    let own = held(&ctx, OWN).await.unwrap_or_default();
    assert_ne!(own.id, format!("{}-0", named(OWN)));
}

#[tokio::test]
async fn each_service_on_a_network_created_again_is_named_and_no_other() {
    let networks = Networked::holding(&[(crate::PRODUCT, OWN)]);
    let engine = lemonfiber_fixtures::support::Reporting::holding(
        &["prowlarr", "flaresolverr"],
        crate::ports::docker::Lifecycle::Running,
        crate::ports::docker::Health::Healthy,
    )
    .labelled("prowlarr", None, &[&named(OWN)])
    .labelled("flaresolverr", None, &["elsewhere"]);
    let (_dir, ctx) = machine_running("displaced", &networks, engine);

    let networking = super::ensured(&ctx, &topology(&[OWN])).await;

    assert_eq!(
        networking.map(|networking| networking.displaced),
        Ok(vec!["prowlarr".to_owned()])
    );
}

#[tokio::test]
async fn a_rehearsed_start_names_what_it_would_create_and_creates_nothing() {
    let networks = Networked::holding(&[(crate::PRODUCT, OWN)]);
    let (_dir, ctx) = machine("rehearsed", &networks);

    let report = started(&ctx.clone().rehearsing()).await;

    assert!(networks.made().is_empty());
    assert!(networks.taken().is_empty());
    assert!(report
        .is_ok_and(|report| report.networks_recreated == [named(OWN)]
            && !report.networks_created.is_empty()));
    assert!(kept(&ctx).is_empty());
}

#[tokio::test]
async fn a_network_that_cannot_be_created_refuses_the_start() {
    let networks = Networked::unmaking(Vec::new());
    let (_dir, ctx) = machine("unmade", &networks);

    assert_eq!(
        started(&ctx).await.err(),
        Some(NETWORK_NOT_MADE.to_string())
    );
}

/// Every network a start of the indexer's form creates on an engine holding none, in the
/// order it creates them.
async fn every_network() -> Vec<Network> {
    let networks = Networked::none();
    let (_dir, ctx) = machine("every", &networks);
    let _ = started(&ctx).await;
    ctx.seams
        .networks
        .networks(crate::PRODUCT)
        .await
        .unwrap_or_default()
}

#[tokio::test]
async fn a_network_held_on_the_subnet_another_is_given_is_taken_away_before_any_is_created() {
    let every = every_network().await;
    assert!(every.len() > 1, "{every:?}");
    let first = every.first().cloned().unwrap_or_default();
    let last = every.last().cloned().unwrap_or_default();
    let mut stale = last.clone();
    stale.shape.subnet = Some(format!(
        "{}/{}",
        crate::topology::RANGE.0,
        crate::topology::RANGE.1
    ));
    stale.id = format!("{}-0", last.name);
    assert!(first.shape.subnet.is_some());
    let networks = Networked::with(vec![stale]);
    let (_dir, ctx) = machine("moved", &networks);

    let report = started(&ctx).await;

    assert_eq!(networks.taken(), std::slice::from_ref(&last.name));
    assert_eq!(
        report.map(|report| report.networks_recreated),
        Ok(vec![last.name.clone()])
    );
}

#[tokio::test]
async fn a_network_held_on_the_subnet_another_s_name_gives_keeps_it_and_the_other_takes_another() {
    let every = every_network().await;
    let first = every.first().cloned().unwrap_or_default();
    let last = every.last().cloned().unwrap_or_default();
    let mut kept = last.clone();
    kept.shape.subnet.clone_from(&first.shape.subnet);
    kept.id = format!("{}-0", last.name);
    let networks = Networked::with(vec![kept.clone()]);
    let (_dir, ctx) = machine("kept-where-held", &networks);

    let report = started(&ctx).await;

    assert!(networks.taken().is_empty(), "{:?}", networks.taken());
    assert!(report.is_ok_and(|report| report.networks_recreated.is_empty()));
    assert_eq!(
        held(&ctx, &last.declared)
            .await
            .map(|now| (now.id, now.shape.subnet)),
        Some((kept.id, first.shape.subnet.clone()))
    );
    let moved = held(&ctx, &first.declared).await;
    assert!(moved.is_some_and(|moved| moved.shape.subnet != first.shape.subnet));
}

/// A topology of `declared` alone, each network internal, its subnets given as a start
/// gives them.
fn topology(declared: &[&str]) -> crate::topology::Topology {
    crate::topology::Topology {
        networks: declared
            .iter()
            .map(|name| {
                (
                    (*name).to_owned(),
                    crate::topology::Network {
                        internal: true,
                        masquerade: false,
                        fixed: None,
                        subnet: None,
                    },
                )
            })
            .collect(),
        placed: std::collections::BTreeMap::new(),
    }
    .holding(&std::collections::BTreeMap::new())
}

/// A name sorting before `held` whose subnet, given alone, is the one `held`'s is.
fn meeting(held: &str) -> String {
    let subnet = |name: &str| {
        topology(&[name])
            .networks
            .get(name)
            .and_then(|network| network.subnet)
    };
    (0..u32::MAX)
        .map(|at| format!("link__a{at}"))
        .find(|name| subnet(name) == subnet(held))
        .unwrap_or_default()
}

#[tokio::test]
async fn a_network_coming_to_a_held_ones_slot_creates_no_held_network_again() {
    let coming = meeting(OWN);
    let networks = Networked::none();
    let (_dir, ctx) = machine("coming", &networks);
    let first = super::ensured(&ctx, &topology(&[OWN])).await;
    let was = held(&ctx, OWN).await;

    let second = super::ensured(&ctx, &topology(&[OWN, &coming])).await;

    assert!(first.is_ok());
    assert!(networks.taken().is_empty(), "{:?}", networks.taken());
    assert_eq!(
        second.map(|second| (second.created, second.recreated)),
        Ok((vec![named(&coming)], Vec::new()))
    );
    assert_eq!(held(&ctx, OWN).await, was);
    assert_ne!(
        held(&ctx, &coming).await.and_then(|one| one.shape.subnet),
        was.and_then(|one| one.shape.subnet)
    );
}

#[tokio::test]
async fn a_network_the_project_no_longer_declares_gives_up_its_subnet_before_any_is_created() {
    let every = every_network().await;
    assert!(!every.is_empty());
    let first = every.first().cloned().unwrap_or_default();
    let gone = Network {
        name: named("gone__left"),
        declared: "gone__left".to_owned(),
        id: format!("{}-0", named("gone__left")),
        shape: first.shape,
        gateway: None,
    };
    let networks = Networked::with(vec![gone]);
    let (_dir, ctx) = machine("undeclared", &networks);

    let report = started(&ctx).await;

    assert_eq!(networks.taken(), [named("gone__left")]);
    assert_eq!(
        report.map(|report| report.swept),
        Ok(vec![named("gone__left")])
    );
}

#[tokio::test]
async fn a_start_whose_networks_cannot_be_kept_for_the_fence_is_refused() {
    let networks = Networked::none();
    let (_dir, ctx) = machine("unkept", &networks);
    let kept_at = crate::app::project::directory(&ctx)
        .map(|directory| directory.join(crate::fence::NETWORKS_HELD));
    assert!(kept_at
        .as_ref()
        .is_some_and(|at| std::fs::create_dir_all(at).is_ok()));

    assert_eq!(
        started(&ctx).await.err(),
        Some(NETWORK_NOT_MADE.to_string())
    );
}

#[tokio::test]
async fn a_network_another_program_holds_on_the_subnet_one_is_given_refuses_the_start() {
    let every = every_network().await;
    let first = every.first().cloned().unwrap_or_default();
    let elsewhere = Network {
        name: "elsewhere_bridge".to_owned(),
        declared: "bridge".to_owned(),
        id: "elsewhere-0".to_owned(),
        shape: first.shape,
        gateway: None,
    };
    let networks = Networked::with(vec![elsewhere]);
    let (_dir, ctx) = machine("elsewhere", &networks);

    assert_eq!(
        started(&ctx).await.err(),
        Some(NETWORK_NOT_MADE.to_string())
    );
}

#[tokio::test]
async fn a_teardown_says_each_network_that_would_not_go_and_keeps_it() {
    let networks = Networked::refusing(&[(crate::PRODUCT, OWN)]);
    let (_dir, mut ctx) = machine("kept-down", &networks);
    ctx.seams.engine = Arc::new(lemonfiber_fixtures::support::Reporting::holding(
        &[],
        crate::ports::docker::Lifecycle::Running,
        crate::ports::docker::Health::Healthy,
    ));

    super::taken_down(&ctx, &topology(&[OWN])).await;

    assert!(networks.taken().is_empty());
    assert!(held(&ctx, OWN).await.is_some());
}

#[tokio::test]
async fn a_teardown_that_left_no_container_takes_every_declared_network_away() {
    let networks = Networked::holding(&[(crate::PRODUCT, OWN), (crate::PRODUCT, "own__other")]);
    let (_dir, mut ctx) = machine("taken-down", &networks);
    ctx.seams.engine = Arc::new(lemonfiber_fixtures::support::Reporting::holding(
        &[],
        crate::ports::docker::Lifecycle::Running,
        crate::ports::docker::Health::Healthy,
    ));

    super::taken_down(&ctx, &topology(&[OWN])).await;

    assert_eq!(networks.taken(), [named(OWN)]);
}

#[tokio::test]
async fn a_network_held_in_another_shape_that_will_not_go_refuses_the_start() {
    let networks = Networked::refusing(&[(crate::PRODUCT, OWN)]);
    let (_dir, ctx) = machine("unremoved", &networks);

    assert_eq!(
        started(&ctx).await.err(),
        Some(NETWORK_NOT_MADE.to_string())
    );
}

#[tokio::test]
async fn an_engine_that_cannot_list_its_networks_refuses_the_start() {
    let networks = Networked::unlisting();
    let (_dir, ctx) = machine("unlisted", &networks);

    assert_eq!(
        started(&ctx).await.err(),
        Some(NETWORK_NOT_MADE.to_string())
    );
}
