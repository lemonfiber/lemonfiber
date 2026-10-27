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
use crate::ports::service::Invited as Recorded;
use crate::quality::Preset;
use crate::stack::Source;
use crate::test_support::{
    a_context, media, nowhere, refused, spoke, Recording, Reporting, Scripted,
};
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
        Ok(Outcome::Invitation(report)) => Some(report),
        _ => None,
    }
}

/// The removal a dispatch answered with, where it answered with one.
///
/// Named for the same reason `invited` is: a `matches!` spanning lines inside an
/// assertion leaves the gate a line it cannot see executed.
fn removed(said: &Result<Outcome, Box<super::Problem>>) -> Option<&crate::model::HouseholdRemoval> {
    match said {
        Ok(Outcome::Removal(report)) => Some(report),
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
        crate::config::IDENTITY_SOURCE_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    env
}

/// A media server holding `members`, recording `since` as when each invitation was made,
/// making account `9` for ana when asked to invite somebody, and taking every write.
fn holding(since: &[Recorded], members: &[crate::ports::service::Member]) -> Arc<Fake> {
    Fake::by_path_in_turn(answering(since, members))
}

/// The routes [`holding`] answers, for a test that adds the request service's.
fn answering(
    since: &[Recorded],
    members: &[crate::ports::service::Member],
) -> Vec<(&'static str, Vec<Answer>)> {
    media::household(members, &media::member("9", "ana", false), since)
}

/// The media server's record of account `id` being made at `at`.
fn made(id: &str, at: String) -> Vec<Recorded> {
    vec![Recorded {
        member: id.to_owned(),
        at,
    }]
}

/// What the media server records of account `id` being made an hour ago, by the test clock.
fn made_lately(id: &str) -> Vec<Recorded> {
    made(id, a_context().build().hours_ago(1))
}

/// What the media server records of account `id` being made long before any window.
fn made_long_ago(id: &str) -> Vec<Recorded> {
    made(id, "2000-01-01T00:00:00Z".to_owned())
}

/// A context over the shipped stack with its media server moved into its plugin, the env
/// file `env` and a household host, answering over `http`.
fn offered_at(env: &std::path::Path, http: Arc<Fake>) -> Ctx {
    media::reaching(
        a_context()
            .settings(Settings {
                env_file: Some(env.to_path_buf()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http),
    )
}

/// Whether `asked` went to the media server's `operation` of `identity.source`.
fn asked_to(asked: &crate::ports::http::Request, operation: &str) -> bool {
    asked.url.ends_with(media::identity(operation))
}

/// The identity operations that change the household.
const HOUSEHOLD_WRITES: [&str; 6] = [
    "invite",
    "claimable",
    "allow",
    "unclaim",
    "withdraw",
    "suspend",
];

/// What a version report looks like when the engine said `compose`.
///
/// Tests assert against the whole outcome rather than picking it apart. A
/// destructuring assertion needs a branch for the case that cannot happen,
/// and that branch is a line no test can ever cover.
fn reported(compose: Option<&str>) -> Outcome {
    Outcome::Version(VersionReport {
        binary: env!("CARGO_PKG_VERSION").to_owned(),
        supported_schema: vec![1],
        stack: crate::test_support::shipped_stack_version(),
        compose: compose.map(str::to_owned),
        attestation_status: crate::device_keys::attestation::status::taken(),
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
            | Outcome::Playing(_)
            | Outcome::Title(_)
            | Outcome::PartWay(_)
            | Outcome::Granted(_)
            | Outcome::Watched(_)
            | Outcome::Askable(_)
            | Outcome::Asked(_)
            | Outcome::FrontDoor(_)
            | Outcome::News(_)
            | Outcome::Stuck(_)
            | Outcome::Word(_)
            | Outcome::Glossary(_)
            | Outcome::Clients(_)
            | Outcome::Invitation(_)
            | Outcome::Handoff(_)
            | Outcome::Pairing(_)
            | Outcome::Certificate(_)
            | Outcome::Removal(_)
            | Outcome::Catalogue(_)
            | Outcome::Wiring(_)
            | Outcome::Substitution(_)
            | Outcome::Settings(_)
            | Outcome::Outbound(_)
            | Outcome::Plugins(_)
            | Outcome::Provenance(_)
            | Outcome::Credentials(_)
            | Outcome::Stored(_)
            | Outcome::SelfUpdate(_)
            | Outcome::Space(_)
            | Outcome::StopSeeding(_)
            | Outcome::Bandwidth(_)
            | Outcome::Pausing(_)
            | Outcome::Minted(_)
            | Outcome::KeyClaim(_)
            | Outcome::Keys(_)
            | Outcome::DeviceKey(_)
            | Outcome::DeviceKeys(_)
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
            | Outcome::Bundle(_)
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
        .with_http(Fake::silent())
}

/// Everything the `library` form declares.
const LIBRARY: [&str; 8] = [
    "jellyfin",
    "door",
    "seerr",
    "request-gate",
    "decline",
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
            | Outcome::Playing(_)
            | Outcome::Title(_)
            | Outcome::PartWay(_)
            | Outcome::Granted(_)
            | Outcome::Watched(_)
            | Outcome::Askable(_)
            | Outcome::Asked(_)
            | Outcome::FrontDoor(_)
            | Outcome::News(_)
            | Outcome::Stuck(_)
            | Outcome::Word(_)
            | Outcome::Glossary(_)
            | Outcome::Clients(_)
            | Outcome::Invitation(_)
            | Outcome::Handoff(_)
            | Outcome::Pairing(_)
            | Outcome::Certificate(_)
            | Outcome::Removal(_)
            | Outcome::Catalogue(_)
            | Outcome::Wiring(_)
            | Outcome::Substitution(_)
            | Outcome::Settings(_)
            | Outcome::Outbound(_)
            | Outcome::Plugins(_)
            | Outcome::Provenance(_)
            | Outcome::Credentials(_)
            | Outcome::Stored(_)
            | Outcome::SelfUpdate(_)
            | Outcome::Space(_)
            | Outcome::StopSeeding(_)
            | Outcome::Bandwidth(_)
            | Outcome::Pausing(_)
            | Outcome::Minted(_)
            | Outcome::KeyClaim(_)
            | Outcome::Keys(_)
            | Outcome::DeviceKey(_)
            | Outcome::DeviceKeys(_)
            | Outcome::Doctor(_)
            | Outcome::Repair(_)
            | Outcome::Undo(_)
            | Outcome::Seed(_)
            | Outcome::Reset(_)
            | Outcome::Uninstall(_)
            | Outcome::Wizard(_)
            | Outcome::Update(_)
            | Outcome::Backup(_)
            | Outcome::Bundle(_)
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
