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

use std::path::PathBuf;
use std::sync::Arc;

use common::stack::project;
use lemonfiber_core::app::{dispatch, Command, Ctx, MigrateAction, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::MigrationReport;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::filesystem::{FsKind, StorageFacts};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted, SeedFs};

/// A machine whose engine answers with the given containers and images.
fn ctx(engine: Reporting, images: Arc<Pulled>) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(engine),
        lemonfiber_fixtures::ports::Stopped::today(),
        Arc::new(SeedFs::keyed(None, None).with_facts(StorageFacts {
            point: PathBuf::from("/srv/media"),
            kind: FsKind::Linking("apfs".to_owned()),
            removable: false,
            available: 100,
            total: 1_000,
        })),
        Source::External(project()),
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
async fn a_machine_with_nothing_else_on_it_says_it_looked() {
    let found = surveyed(&ctx(Reporting::absent(), Pulled::holding(Vec::new()))).await;
    let read = found.as_ref().map(|report| report.read);
    assert_eq!(read, Some(true), "{found:?}");
    let standing = found.as_ref().map(|report| report.standing.len());
    assert_eq!(standing, Some(0), "{found:?}");
}
