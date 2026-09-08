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

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::stack::project;
use lemonfiber_core::app::{dispatch, Command, Ctx, MigrateAction, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::AdoptReport;
use lemonfiber_core::model::MigrationReport;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::filesystem::{FsKind, StorageFacts};
use lemonfiber_core::ports::Runner;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{spoke, Recording, Reporting, Scripted, SeedFs};

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
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(engine),
        lemonfiber_fixtures::ports::Stopped::today(),
        files,
        Source::External(project()),
        Settings {
            project: "lemonfiber".to_owned(),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_images(images)
}

/// The same machine again, with the programs it runs answered by a given runner.
///
/// Apart from the others because a claim about what a survey *did not* run cannot be
/// made from what came back: it has to be asked of the thing that would have run it.
fn driven(engine: Reporting, images: Arc<Pulled>, stack: Source, runner: Arc<dyn Runner>) -> Ctx {
    Ctx::new(
        runner,
        Arc::new(engine),
        lemonfiber_fixtures::ports::Stopped::today(),
        Arc::new(SeedFs::keyed(None, None).with_facts(StorageFacts {
            point: PathBuf::from("/srv/media"),
            kind: FsKind::Linking("apfs".to_owned()),
            removable: false,
            available: 100,
            total: 1_000,
        })),
        stack,
        Settings {
            project: "lemonfiber".to_owned(),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_images(images)
}

/// What the survey answered, or nothing where the command refused.
async fn surveyed(ctx: &Ctx) -> Option<MigrationReport> {
    match dispatch(Command::Migrate(MigrateAction::Survey), ctx).await {
        Ok(Outcome::Migration(report)) => Some(report),
        _ => None,
    }
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

#[tokio::test]
async fn the_survey_reaches_the_command_and_names_the_project_it_found() {
    let images = Pulled::holding(vec![Pulled::image("sonarr", 400, &["media"])]);
    let found = surveyed(&ctx(somebody_elses(), images)).await;
    let named = found
        .as_ref()
        .and_then(|report| report.standing.first())
        .map(|standing| standing.project.clone());
    assert_eq!(named, Some("media".to_owned()), "{found:?}");
}

#[tokio::test]
async fn a_port_we_want_that_the_existing_stack_holds_is_named_before_anything_is_proposed() {
    let images = Pulled::holding(vec![Pulled::image("sonarr", 400, &["media"])]);
    let found = surveyed(&ctx(somebody_elses(), images)).await;
    let held = found
        .as_ref()
        .and_then(|report| report.conflicts.first())
        .map(|clash| clash.held_by.clone());
    assert_eq!(held, Some("media/sonarr".to_owned()), "{found:?}");
}

#[tokio::test]
async fn an_engine_that_will_not_list_reports_it_could_not_look() {
    let images = Pulled::holding(vec![Pulled::image("sonarr", 400, &["media"])]);
    let found = surveyed(&ctx(Reporting::absent(), images)).await;
    let read = found.as_ref().map(|report| report.read);
    assert_eq!(read, Some(false), "{found:?}");
}

#[tokio::test]
async fn an_engine_that_will_not_say_what_it_pulled_reports_it_could_not_look() {
    let refused = Pulled::unreachable("no daemon here");
    let found = surveyed(&ctx(somebody_elses(), refused)).await;
    let read = found.as_ref().map(|report| report.read);
    assert_eq!(read, Some(false), "{found:?}");
}

#[tokio::test]
async fn a_stack_whose_manifest_cannot_be_read_reports_it_could_not_look() {
    let images = Pulled::holding(vec![Pulled::image("sonarr", 400, &["media"])]);
    let nowhere = Source::External(Path::new("/nowhere-at-all"));
    let found = surveyed(&over(somebody_elses(), images, nowhere)).await;
    let read = found.as_ref().map(|report| report.read);
    assert_eq!(read, Some(false), "{found:?}");
}

#[tokio::test]
async fn a_machine_with_nothing_else_on_it_says_it_looked() {
    let found = surveyed(&ctx(Reporting::absent(), Pulled::holding(Vec::new()))).await;
    let read = found.as_ref().map(|report| report.read);
    assert_eq!(read, Some(true), "{found:?}");
    let standing = found.as_ref().map(|report| report.standing.len());
    assert_eq!(standing, Some(0), "{found:?}");
}

/// A survey is a read. Nothing it does may reach the operator's running stack, because
/// a migration they abandon halfway has to leave them exactly what they had.
#[tokio::test]
async fn a_survey_runs_no_program_at_all() {
    let watching = Arc::new(Recording::answering(Ok(spoke(""))));
    let images = Pulled::holding(vec![Pulled::image("sonarr", 400, &["media"])]);
    let ctx = driven(
        somebody_elses(),
        images,
        Source::External(project()),
        Arc::clone(&watching) as Arc<dyn Runner>,
    );

    let found = surveyed(&ctx).await;
    assert!(found.is_some(), "the survey answered");

    // Everything it was handed, not one word at a time: the invocation a narrower
    // question forgot to ask about is the one that would have stopped their stack.
    let ran = watching.seen();
    assert!(ran.is_empty(), "a survey ran a program: {ran:?}");
}

/// A layout whose data sits on two filesystems cannot hardlink between them, and the
/// survey has to say so from what the engine actually reported being mounted.
#[tokio::test]
async fn a_layout_across_two_filesystems_is_reported_with_its_cost_and_a_remedy() {
    let images = Pulled::holding(vec![Pulled::image("sonarr", 400, &["media"])]);
    let engine = mounting(somebody_elses(), &["/srv/media", "/mnt/downloads"]);

    let split = SeedFs::keyed(None, None)
        .with_facts(facts("/srv"))
        .with_facts_under("/mnt", facts("/mnt"));

    let ctx = over_files(engine, images, Arc::new(split));
    let found = surveyed(&ctx).await.and_then(|report| report.linking);
    assert_eq!(
        found.as_ref().map(|read| (read.links, read.forced)),
        Some((false, false)),
        "two filesystems is a finding the operator decides about: {found:?}"
    );
    let said = found
        .as_ref()
        .map(|read| read.because.clone())
        .unwrap_or_default();
    assert!(said.contains("/mnt") && said.contains("/srv"), "{said}");
    let remedy = found.map(|read| read.remedy).unwrap_or_default();
    assert!(!remedy.is_empty(), "a remedy is offered");
}

/// One filesystem is the ordinary case and is not worth telling anybody about.
#[tokio::test]
async fn a_layout_on_one_filesystem_is_not_reported_at_all() {
    let images = Pulled::holding(vec![Pulled::image("sonarr", 400, &["media"])]);
    let engine = mounting(somebody_elses(), &["/srv/media", "/srv/downloads"]);
    let one = SeedFs::keyed(None, None).with_facts(facts("/srv"));

    let ctx = over_files(engine, images, Arc::new(one));
    let found = surveyed(&ctx).await.and_then(|report| report.linking);
    assert!(found.is_none(), "{found:?}");
}

/// A scratch environment file adopting can record its answer in.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-adopt-{}-{name}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = std::fs::remove_file(&env);
    env
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
    match dispatch(Command::Migrate(MigrateAction::Adopt { confirmed }), ctx).await {
        Ok(Outcome::Adoption(report)) => Some(report),
        _ => None,
    }
}

#[tokio::test]
async fn a_database_a_later_version_wrote_is_refused_rather_than_opened() {
    let found = adopting(&theirs("9.9.9", None), true).await;
    let refused = found.as_ref().and_then(|read| read.refused.clone());
    assert!(
        refused.is_some_and(|said| said.contains("later version")),
        "{found:?}"
    );
    assert_eq!(found.map(|read| read.adopted), Some(false));
}

#[tokio::test]
async fn adopting_unconfirmed_says_what_it_would_do_and_writes_nothing() {
    let env = scratch("rehearsed");
    let found = adopting(&theirs("4.0.0", Some(env.clone())), false).await;
    assert_eq!(
        found.as_ref().map(|read| (read.rehearsed, read.adopted)),
        Some((true, false)),
        "{found:?}"
    );
    assert!(!env.exists(), "a rehearsal wrote {}", env.display());
}

#[tokio::test]
async fn an_upgrade_names_the_service_and_where_to_back_it_up_before_confirming() {
    let env = scratch("named");
    let engine = mounting(somebody_elses(), &["/srv/media"]);
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.0",
        400,
        &["media"],
    )]);
    let mut ctx = over(engine, images, Source::External(project()));
    ctx.settings.env_file = Some(env);

    let found = adopting(&ctx, false).await;
    let upgrading = found
        .as_ref()
        .and_then(|read| read.upgrades.first().map(|one| one.service.clone()));
    assert_eq!(upgrading, Some("sonarr".to_owned()), "{found:?}");
    let paths = found.map(|read| read.back_up).unwrap_or_default();
    assert!(
        paths.iter().any(|path| path.contains("/srv/media")),
        "{paths:?}"
    );
}

#[tokio::test]
async fn confirming_records_the_project_lemonfiber_now_manages() {
    let env = scratch("adopted");
    let found = adopting(&theirs("4.0.15", Some(env.clone())), true).await;
    assert_eq!(
        found
            .as_ref()
            .map(|read| (read.adopted, read.project.clone())),
        Some((true, Some("media".to_owned()))),
        "{found:?}"
    );
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(written.contains("LEMONFIBER_PROJECT=media"), "{written}");
}

#[tokio::test]
async fn a_machine_with_nothing_of_ours_on_it_has_nothing_to_adopt() {
    let images = Pulled::holding(Vec::new());
    let ctx = over(Reporting::absent(), images, Source::External(project()));
    let found = adopting(&ctx, true).await;
    let refused = found.and_then(|read| read.refused);
    assert!(refused.is_some(), "nothing to take over is a refusal");
}

/// Adopting is the operator's explicit act, so nowhere to record it is reported rather
/// than shrugged off — an answer that quietly did not persist would leave them
/// believing lemonfiber manages a stack it does not.
#[tokio::test]
async fn adopting_with_nowhere_to_record_it_says_so_rather_than_claiming_it_worked() {
    let asked = Command::Migrate(MigrateAction::Adopt { confirmed: true });
    let refused = dispatch(asked, &theirs("4.0.15", None)).await;
    assert!(refused.is_err(), "{refused:?}");
}

/// The same where there is somewhere but it cannot be written: a file standing where
/// the directory would go.
#[tokio::test]
async fn adopting_that_cannot_write_its_answer_reports_the_failure() {
    let blocked = std::env::temp_dir().join(format!("lemonfiber-blocked-{}", std::process::id()));
    let _ = std::fs::write(&blocked, "not a directory");
    let asked = Command::Migrate(MigrateAction::Adopt { confirmed: true });
    let ctx = theirs("4.0.15", Some(blocked.join(".env")));
    let refused = dispatch(asked, &ctx).await;
    let _ = std::fs::remove_file(&blocked);
    assert!(refused.is_err(), "{refused:?}");
}
