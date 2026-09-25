//! The survey that reads what is already on a machine, dispatched as the surfaces reach it.
//!
//! Beside the crate rather than inside it because the dispatcher is compiled twice —
//! once with the crate's own tests and once without — so an arm exercised only in-crate
//! has its coverage counted from the copy that never ran.
//!
//! What is asserted here is the half the pure survey cannot: that the command routes,
//! and that each way of failing to look reports it could not look rather than reporting
//! an empty machine. The difference decides whether it is safe to stand a stack up over
//! somebody's library, so every seam that can refuse is driven to refuse.

use std::path::PathBuf;
use std::sync::Arc;

use crate::common;
use common::stack::project;
use lemonfiber_core::app::{dispatch, Command, Ctx, MigrateAction, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::migration::mode::Mode;
use lemonfiber_core::model::{AdoptReport, BesideReport, ReplaceReport};
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::filesystem::{FsKind, StorageFacts};
use lemonfiber_core::ports::http::Method;
use lemonfiber_core::ports::Runner;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted, SeedFs};

/// A machine whose engine answers with the given containers and images.
fn ctx(engine: Reporting, images: Arc<Pulled>) -> Ctx {
    over(engine, images, Source::External(project()))
}

/// The same machine, reading its stack from somewhere named.
fn over(engine: Reporting, images: Arc<Pulled>, stack: Source) -> Ctx {
    driven(engine, images, stack, Arc::new(Scripted(Ok(spoke("")))))
}

/// A filesystem whose paths under `point` all belong to that one mount.
fn facts(point: &str) -> StorageFacts {
    StorageFacts {
        point: PathBuf::from(point),
        kind: FsKind::Linking("apfs".to_owned()),
        removable: false,
        available: 100,
        total: 1_000,
    }
}

/// The same machine, reading its filesystem through a given fake.
fn over_files(engine: Reporting, images: Arc<Pulled>, files: Arc<SeedFs>) -> Ctx {
    lemonfiber_testing::a_context()
        .engine(Arc::new(engine))
        .filesystem(files)
        .settings(Settings {
            project: "lemonfiber".to_owned(),
            ..Settings::default()
        })
        .build()
        .with_images(images)
}

/// The same machine again, with the programs it runs answered by a given runner.
///
/// Apart from the others because a claim about what a survey *did not* run cannot be
/// made from what came back: it has to be asked of the thing that would have run it.
fn driven(engine: Reporting, images: Arc<Pulled>, stack: Source, runner: Arc<dyn Runner>) -> Ctx {
    lemonfiber_testing::a_context()
        .runner(runner)
        .engine(Arc::new(engine))
        .filesystem(Arc::new(SeedFs::keyed(None, None).with_facts(
            StorageFacts {
                point: PathBuf::from("/srv/media"),
                kind: FsKind::Linking("apfs".to_owned()),
                removable: false,
                available: 100,
                total: 1_000,
            },
        )))
        .over(stack)
        .settings(Settings {
            project: "lemonfiber".to_owned(),
            ..Settings::default()
        })
        .build()
        .with_images(images)
}

/// The same engine, with the given host paths mounted into its containers.
fn mounting(engine: Reporting, paths: &[&str]) -> Reporting {
    engine.mounting(&paths.iter().map(PathBuf::from).collect::<Vec<_>>())
}

/// An engine holding one service, under a project that is not lemonfiber's.
fn somebody_elses() -> Reporting {
    Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
        .belonging_to("media")
        .publishing(&[("sonarr", "127.0.0.1", 8989)])
}

/// A scratch environment file adopting can record its answer in.
fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::new(name).within(".env")
}

/// A machine whose existing project runs a service lemonfiber knows, at a given version.
fn theirs(version: &str, env: Option<PathBuf>) -> Ctx {
    let images = Pulled::holding(vec![Pulled::image(
        &format!("lscr.io/linuxserver/sonarr:{version}"),
        400,
        &["media"],
    )]);
    let mut ctx = over(somebody_elses(), images, Source::External(project()));
    ctx.settings.env_file = env;
    ctx
}

/// What adopting answered, or nothing where it refused to answer at all.
async fn adopting(ctx: &Ctx, confirmed: bool) -> Option<AdoptReport> {
    match dispatch(
        Command::Migrate(MigrateAction::Act {
            mode: Mode::Adopt,
            confirmed,
        }),
        ctx,
    )
    .await
    {
        Ok(Outcome::Adoption(report)) => Some(report),
        _ => None,
    }
}

/// What standing beside answered, or nothing where it refused to answer at all.
async fn standing(ctx: &Ctx, confirmed: bool) -> Option<BesideReport> {
    match dispatch(
        Command::Migrate(MigrateAction::Act {
            mode: Mode::Beside,
            confirmed,
        }),
        ctx,
    )
    .await
    {
        Ok(Outcome::Beside(report)) => Some(report),
        _ => None,
    }
}

/// What standing in place of it answered.
async fn replacing(ctx: &Ctx, confirmed: bool) -> Option<ReplaceReport> {
    match dispatch(
        Command::Migrate(MigrateAction::Act {
            mode: Mode::Replace,
            confirmed,
        }),
        ctx,
    )
    .await
    {
        Ok(Outcome::Replacement(report)) => Some(report),
        _ => None,
    }
}

/// Their sonarr, on a port of its own so the two stacks can be told apart.
fn both_stacks() -> (Reporting, Arc<Pulled>) {
    let engine = Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
        .belonging_to("media")
        .publishing(&[("sonarr", "127.0.0.1", 18989)])
        .mounting(&[PathBuf::from("/their/sonarr")]);
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.15",
        400,
        &["media"],
    )]);
    (engine, images)
}

/// A transport answering as two stacks at once, told apart by the port asked.
fn two_stacks() -> Arc<lemonfiber_fixtures::http::Fake> {
    use lemonfiber_fixtures::http::{Answer, Fake};
    Fake::by_route(vec![
        // Theirs: one series, following a profile it numbered 1.
        (
            Method::Get,
            "18989/api/v3/qualityprofile",
            Answer::reply(200, r#"[{"id":1,"name":"HD"}]"#),
        ),
        (
            Method::Get,
            "18989/api/v3/series",
            Answer::reply(
                200,
                r#"[{"id":5,"title":"Taskmaster","qualityProfileId":1,"rootFolderPath":"/data/tv"}]"#,
            ),
        ),
        (
            Method::Get,
            "18989/api/v3/indexer",
            Answer::reply(200, "[]"),
        ),
        // Ours: the same profile, numbered differently, and nothing followed yet.
        (
            Method::Get,
            ":8989/api/v3/qualityprofile",
            Answer::reply(200, r#"[{"id":7,"name":"HD"}]"#),
        ),
        (Method::Get, ":8989/api/v3/series", Answer::reply(200, "[]")),
        (
            Method::Get,
            ":8989/api/v3/indexer",
            Answer::reply(200, "[]"),
        ),
        (
            Method::Post,
            ":8989/api/v3/series",
            Answer::reply(201, "{}"),
        ),
    ])
}

/// A machine holding both stacks, reached through the given transport.
fn importing(http: Arc<lemonfiber_fixtures::http::Fake>) -> Ctx {
    let (engine, images) = both_stacks();
    lemonfiber_testing::a_context()
        .engine(Arc::new(engine))
        .filesystem(Arc::new(SeedFs::keyed(
            Some("<Config><ApiKey>the-key</ApiKey></Config>"),
            None,
        )))
        .settings(Settings {
            project: "lemonfiber".to_owned(),
            stack_dir: Some(PathBuf::from("/srv/lemonfiber")),
            ..Settings::default()
        })
        .build()
        .with_images(images)
        .with_http(http)
}

/// What carrying answered.
async fn carrying(ctx: &Ctx, confirmed: bool) -> Option<lemonfiber_core::model::ImportReport> {
    match dispatch(
        Command::Migrate(MigrateAction::Act {
            mode: Mode::Import,
            confirmed,
        }),
        ctx,
    )
    .await
    {
        Ok(Outcome::Import(report)) => Some(report),
        _ => None,
    }
}

mod adopting;
mod beside;
mod carrying;
mod replacing;
mod surveying;
