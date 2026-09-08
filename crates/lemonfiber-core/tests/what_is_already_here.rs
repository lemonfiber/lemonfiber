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
