use std::sync::Arc;

use crate::doctor::Narrowing;

use super::{
    dispatch, pull_progress, AlertAction, Allowance, Answer as Ruling, Asking, BandwidthAsked,
    Chosen, Command, Ctx, Decision, MigrateAction, Outcome, QualityAction, Removing, Setting,
    SetupAction, Waiting,
};
use crate::config::Settings;
use crate::docker::{Condition, State as ServiceState};
use crate::doctor::Category;
use crate::migration::mode::Mode;
use crate::model::InvitationStanding;
use crate::model::VersionReport;
use crate::ports::docker::{Engine, Failure as EngineFailure, Health, Lifecycle, LogQuery};
use crate::ports::process::{Failure, Output, Progress};
use crate::quality::Preset;
use crate::stack::Source;
use crate::test_support::{a_context, nowhere, refused, spoke, Recording, Reporting, Scripted};
use lemonfiber_fixtures::http::{Answer, Fake};
use std::time::Duration;

fn ctx(scripted: Result<Output, Failure>) -> Ctx {
    a_context()
        .runner(Arc::new(Scripted(scripted)))
        .engine(Arc::new(Reporting::default()))
        .build()
}

/// The invitation an answer carries, if it carried one.
///
/// A named reader rather than a `matches!` spanning lines inside an assertion:
/// that shape leaves the gate a line it cannot see executed, and it reads worse
/// besides.
fn invited(made: &Result<Outcome, Box<super::Problem>>) -> Option<&crate::model::Invitation> {
    match made {
        Ok(Outcome::Invited(report)) => Some(report),
        _ => None,
    }
}

/// The removal a dispatch answered with, where it answered with one.
///
/// Named for the same reason `invited` is: a `matches!` spanning lines inside an
/// assertion leaves the gate a line it cannot see executed.
fn removed(said: &Result<Outcome, Box<super::Problem>>) -> Option<&crate::model::HouseholdRemoval> {
    match said {
        Ok(Outcome::Removed(report)) => Some(report),
        _ => None,
    }
}

/// A scratch environment file holding the media server's recorded password.
fn recorded_admin(name: &str) -> std::path::PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("invite-{name}")).kept();
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = crate::config::store::set(
        &env,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    env
}

/// A media server holding one account, and answering every call this makes.
///
/// `Users` is what the household read returns, and `log` what the record of
/// account-making returns — the two halves that decide what is already here.
fn holding(log: &'static str, users: &'static str) -> std::sync::Arc<Fake> {
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![
                signed_in.clone(),
                signed_in.clone(),
                signed_in.clone(),
                signed_in,
            ],
        ),
        ("/System/ActivityLog", vec![Answer::reply(200, log)]),
        (
            "/Users/New",
            vec![Answer::reply(
                200,
                r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
            )],
        ),
        ("/Users/7", vec![Answer::reply(204, "")]),
        ("/Users", vec![Answer::reply(200, users)]),
    ])
}

/// What a version report looks like when the engine said `compose`.
///
/// Tests assert against the whole outcome rather than picking it apart. A
/// destructuring assertion needs a branch for the case that cannot happen,
/// and that branch is a line no test can ever cover.
fn reported(compose: Option<&str>) -> Outcome {
    Outcome::Version(VersionReport {
        binary: env!("CARGO_PKG_VERSION").to_owned(),
        supported_schema: vec![1],
        stack: "0.1.0".to_owned(),
        compose: compose.map(str::to_owned),
        changelog: crate::changelog::notes(env!("CARGO_PKG_VERSION")),
    })
}

/// A context that runs against the checked-out stack, in rehearsal.
fn rehearsing(protocols: crate::config::Protocols) -> Ctx {
    let settings = Settings {
        protocols,
        ..Settings::default()
    };
    a_context()
        .engine(Arc::new(Reporting::default()))
        .settings(settings)
        .build()
        .rehearsing()
}

fn report(outcome: Result<Outcome, Box<super::Problem>>) -> Option<crate::model::LifecycleReport> {
    match outcome {
        Ok(Outcome::Lifecycle(report)) => Some(report),
        Ok(
            Outcome::Version(_)
            | Outcome::Alerts(_)
            | Outcome::Migration(_)
            | Outcome::History(_)
            | Outcome::Adoption(_)
            | Outcome::Beside(_)
            | Outcome::Replacement(_)
            | Outcome::Import(_)
            | Outcome::Forms(_)
            | Outcome::Preview(_)
            | Outcome::Config(_)
            | Outcome::Quality(_)
            | Outcome::Upgrade(_)
            | Outcome::Music(_)
            | Outcome::Trace(_)
            | Outcome::Hosting(_)
            | Outcome::Household(_)
            | Outcome::Held(_)
            | Outcome::FrontDoor(_)
            | Outcome::Stuck(_)
            | Outcome::Word(_)
            | Outcome::Glossary(_)
            | Outcome::Clients(_)
            | Outcome::Invited(_)
            | Outcome::Removed(_)
            | Outcome::Catalogue(_)
            | Outcome::Wiring(_)
            | Outcome::Substituted(_)
            | Outcome::Outbound(_)
            | Outcome::Plugins(_)
            | Outcome::Provenance(_)
            | Outcome::Credentials(_)
            | Outcome::Stored(_)
            | Outcome::SelfUpdate(_)
            | Outcome::Space(_)
            | Outcome::Letting(_)
            | Outcome::Bandwidth(_)
            | Outcome::Status(_)
            | Outcome::Doctor(_)
            | Outcome::Repair(_)
            | Outcome::Undo(_)
            | Outcome::Seed(_)
            | Outcome::Reset(_)
            | Outcome::Uninstall(_)
            | Outcome::Wizard(_)
            | Outcome::Update(_)
            | Outcome::Backup(_)
            | Outcome::Support(_)
            | Outcome::Archives(_)
            | Outcome::Restore(_)
            | Outcome::Watch(_)
            | Outcome::Walkthrough(_),
        )
        | Err(_) => None,
    }
}

fn config_scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::unmade(name).within(".env")
}

/// A real run against an engine reporting whatever the test put in it.
fn watching(engine: Reporting) -> Ctx {
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    a_context()
        .engine(Arc::new(engine))
        .settings(settings)
        .build()
        // An HTTP port that answers nothing, so a diagnostic check reaching one — the
        // guide-source probe, a credential — resolves to unreachable rather than the
        // real network. Keeps the doctor tests self-contained and offline.
        .with_http(Fake::scripted(Vec::new()))
}

/// Everything the `library` form declares.
const LIBRARY: [&str; 5] = [
    "jellyfin",
    "seerr",
    "calibre-web-automated",
    "audiobookshelf",
    "navidrome",
];

/// The status a survey produced, as pairs of service and state.
fn stated(outcome: Result<Outcome, Box<super::Problem>>) -> Option<Vec<(String, ServiceState)>> {
    match outcome {
        Ok(Outcome::Status(report)) => Some(
            report
                .services
                .into_iter()
                .map(|service| (service.id, service.state))
                .collect(),
        ),
        Ok(
            Outcome::Version(_)
            | Outcome::Alerts(_)
            | Outcome::Migration(_)
            | Outcome::History(_)
            | Outcome::Adoption(_)
            | Outcome::Beside(_)
            | Outcome::Replacement(_)
            | Outcome::Import(_)
            | Outcome::Forms(_)
            | Outcome::Preview(_)
            | Outcome::Lifecycle(_)
            | Outcome::Config(_)
            | Outcome::Quality(_)
            | Outcome::Upgrade(_)
            | Outcome::Music(_)
            | Outcome::Trace(_)
            | Outcome::Hosting(_)
            | Outcome::Household(_)
            | Outcome::Held(_)
            | Outcome::FrontDoor(_)
            | Outcome::Stuck(_)
            | Outcome::Word(_)
            | Outcome::Glossary(_)
            | Outcome::Clients(_)
            | Outcome::Invited(_)
            | Outcome::Removed(_)
            | Outcome::Catalogue(_)
            | Outcome::Wiring(_)
            | Outcome::Substituted(_)
            | Outcome::Outbound(_)
            | Outcome::Plugins(_)
            | Outcome::Provenance(_)
            | Outcome::Credentials(_)
            | Outcome::Stored(_)
            | Outcome::SelfUpdate(_)
            | Outcome::Space(_)
            | Outcome::Letting(_)
            | Outcome::Bandwidth(_)
            | Outcome::Doctor(_)
            | Outcome::Repair(_)
            | Outcome::Undo(_)
            | Outcome::Seed(_)
            | Outcome::Reset(_)
            | Outcome::Uninstall(_)
            | Outcome::Wizard(_)
            | Outcome::Update(_)
            | Outcome::Backup(_)
            | Outcome::Support(_)
            | Outcome::Archives(_)
            | Outcome::Restore(_)
            | Outcome::Watch(_)
            | Outcome::Walkthrough(_),
        )
        | Err(_) => None,
    }
}

#[tokio::test]
async fn a_context_can_be_told_how_long_to_wait() {
    let ctx = watching(Reporting::default()).with_patience(Duration::from_secs(7));
    assert_eq!(ctx.patience, Duration::from_secs(7));
}

#[tokio::test]
async fn the_engine_these_tests_use_answers_the_whole_port() {
    // Worth asserting rather than assuming. A fake that answers a method
    // more agreeably than a real engine would makes the path it shortcuts
    // untestable, which is how the log stream's own failure case went
    // missing until it was written down here.
    let engine = Reporting::absent();

    let ran = engine.exec("gluetun", &["true".to_owned()]).await;
    assert!(
        matches!(&ran, Err(EngineFailure::NoSuchContainer { name }) if name == "gluetun"),
        "{ran:?}"
    );

    let sampled = engine.stats("lemonfiber").await;
    assert_eq!(
        sampled.ok().map(|mut samples| samples.try_recv().is_err()),
        Some(true),
        "nothing is sampled, and the stream says so by ending"
    );
}

#[test]
fn a_rehearsing_context_changes_nothing_else() {
    let rehearsal = ctx(Ok(spoke(""))).rehearsing();
    assert!(rehearsal.dry_run);
}

mod configuring;
mod diagnosis;
mod invitation_refusals;
mod inviting;
mod kinds;
mod lifecycle;
mod logs;
mod routing;
mod settling;
mod status;
mod switching;
