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
use lemonfiber_contract::capabilities::library::curate;
use lemonfiber_core::app::{dispatch, Command, Ctx, MigrateAction, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::error::Problem;
use lemonfiber_core::migration::mode::Mode;
use lemonfiber_core::model::{AdoptReport, BesideReport, ReplaceReport};
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::filesystem::{FsKind, StorageFacts};
use lemonfiber_core::ports::service::{Carried, QualityProfile};
use lemonfiber_core::ports::Runner;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::serving::Serves;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted, SeedFs};
use lemonfiber_testing::curators::{answered, curating, taken, TELEVISION};

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
        .filesystem(Arc::new(SeedFs::keyed(None).with_facts(StorageFacts {
            point: PathBuf::from("/srv/media"),
            kind: FsKind::Linking("apfs".to_owned()),
            removable: false,
            available: 100,
            total: 1_000,
        })))
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

/// What standing in place of it answered, asked as `asked`, or the problem it was
/// refused with.
async fn replacing_as(
    ctx: &Ctx,
    asked: MigrateAction,
) -> Result<Option<ReplaceReport>, Box<Problem>> {
    dispatch(Command::Migrate(asked), ctx)
        .await
        .map(|outcome| match outcome {
            Outcome::Replacement(report) => Some(report),
            _ => None,
        })
}

/// What standing in place of it answered `offer` with, or the problem it was refused with.
async fn replacing(ctx: &Ctx, offer: Option<&str>) -> Result<Option<ReplaceReport>, Box<Problem>> {
    replacing_as(
        ctx,
        MigrateAction::Replace {
            offer: offer.map(str::to_owned),
        },
    )
    .await
}

/// Their curator, published on every address of the machine on a port of its own.
fn both_stacks() -> (Reporting, Arc<Pulled>) {
    let engine = Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
        .belonging_to("media")
        .publishing(&[("sonarr", "0.0.0.0", 18989)])
        .mounting(&[PathBuf::from("/their/sonarr")]);
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.15",
        400,
        &["media"],
    )]);
    (engine, images)
}

/// The port the engine says the adapter lent to their copy is published on.
const LENT: u16 = 41_234;

/// Where `operation` of `library.curate` is asked of the adapter lent to their copy.
fn lent(operation: &str) -> &'static str {
    Box::leak(
        format!(
            "127.0.0.1:{LENT}{}",
            lemonfiber_contract::path(curate::CAPABILITY, curate::MAJOR, operation)
        )
        .into_boxed_str(),
    )
}

/// What a request for `records` of `sort` posts.
fn of_sort(sort: &str) -> &'static str {
    Box::leak(format!(r#""sort":"{sort}""#).into_boxed_str())
}

/// The series their curator follows, under a profile named as both stacks name it.
fn taskmaster() -> Carried {
    Carried {
        name: "Taskmaster".to_owned(),
        profile: Some("HD".to_owned()),
        folder: Some("/data/tv".to_owned()),
        rest: "{}".to_owned(),
    }
}

/// The routes of the two stacks, each curator asked through the adapter in front of it,
/// theirs holding `theirs` and ours `ours`, and ours taking a record as `takes` answers.
fn stacks(theirs: &[Carried], ours: &[Carried], takes: Answer) -> Vec<Route> {
    let none = || answered(&Vec::<Carried>::new());
    vec![
        (
            ready_path_of_any_adapter(),
            None,
            vec![Answer::reply(200, "{}")],
        ),
        (
            lent("records"),
            Some(of_sort("series")),
            vec![answered(&theirs)],
        ),
        (lent("records"), Some(of_sort("indexer")), vec![none()]),
        (
            curating(TELEVISION, "quality_profiles"),
            None,
            vec![answered(&[QualityProfile {
                id: 7,
                name: "HD".to_owned(),
            }])],
        ),
        (
            curating(TELEVISION, "records"),
            Some(of_sort("series")),
            vec![answered(&ours)],
        ),
        (
            curating(TELEVISION, "records"),
            Some(of_sort("indexer")),
            vec![none()],
        ),
        (curating(TELEVISION, "carry"), None, vec![takes]),
    ]
}

/// Where any adapter says whether its upstream answers.
fn ready_path_of_any_adapter() -> &'static str {
    Box::leak(lemonfiber_contract::adapter::ready_path().into_boxed_str())
}

/// A route of the two stacks: what its address contains, what is posted to it where
/// that tells it apart, and its answers in turn.
type Route = (&'static str, Option<&'static str>, Vec<Answer>);

/// A transport answering as two stacks at once: theirs holding one series and ours
/// nothing yet, taking what is carried.
fn two_stacks() -> Arc<Fake> {
    Fake::by_call(stacks(&[taskmaster()], &[], taken()))
}

/// A machine holding both stacks, reached through the given transport.
fn importing(http: Arc<Fake>) -> Ctx {
    importing_over(
        both_stacks().0,
        Source::External(project()),
        Arc::new(SeedFs::keyed(Some(THEIR_KEY))),
        http,
    )
}

/// Their curator's configuration, carrying a key their copy wrote.
const THEIR_KEY: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

/// A machine whose engine holds what `engine` holds, its stack read from `stack` and its
/// files from `files`, their curator's image pulled, lemonfiber's own television curator
/// moved into its plugin and reached, and an engine serving the adapter lent to their copy
/// on [`LENT`].
fn importing_over(engine: Reporting, stack: Source, files: Arc<SeedFs>, http: Arc<Fake>) -> Ctx {
    static BUILT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nth = BUILT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("importing-{nth}")).kept();
    let _ = std::fs::create_dir_all(&dir);
    let mut ctx = lemonfiber_testing::a_context()
        .engine(Arc::new(engine.alongside(TELEVISION.running())))
        .filesystem(files)
        .over(stack)
        .settings(Settings {
            project: "lemonfiber".to_owned(),
            stack_dir: Some(PathBuf::from("/srv/lemonfiber")),
            env_file: Some(dir.join(".env")),
            ..Settings::default()
        })
        .build()
        .with_images(both_stacks().1)
        .with_http(http);
    ctx.seams.serving = Arc::new(Serves::on(LENT));
    TELEVISION.keyed(ctx)
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
