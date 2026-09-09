//! An uninstall, dispatched and rendered as the surfaces reach it.
//!
//! Beside the crate rather than inside it because the dispatcher is compiled twice —
//! once with the crate's own tests and once without — so an arm exercised only
//! in-crate has its coverage counted from the copy that never ran. What is asserted
//! here is the half a handler test cannot: that the command routes, that the answer
//! carries the kind a script keys off, and that a wait told to wait waits.
//!
//! The wait is elapsed in virtual time. It looks again every ten seconds, and a test
//! that really waited would be one nobody runs.

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use common::stack::project;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, Removing, Waiting};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::filesystem::{Eraser, FsKind, StorageFacts};
use lemonfiber_core::ports::{Narrator, Runner};
use lemonfiber_core::stack::Source;
use lemonfiber_core::uninstall::{Removal, Tier};
use lemonfiber_fixtures::erasing::Erasing;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{spoke, Recording, Reporting, Scripted, SeedFs};
use lemonfiber_fixtures::walking::Walking;
use tokio::sync::Mutex;

/// One image this stack declares, named once so a case does not spell the tag twice.
const SONARR: &str = "lscr.io/linuxserver/sonarr:4.0.15";

/// Everything a run said, in the order it said it.
#[derive(Default)]
struct Heard(Mutex<Vec<String>>);

#[async_trait]
impl Narrator for Heard {
    async fn say(&self, said: &str) {
        self.0.lock().await.push(said.to_owned());
    }
}

impl Heard {
    /// What it has heard so far.
    async fn said(&self) -> Vec<String> {
        self.0.lock().await.clone()
    }
}

/// A context over the stack this repository carries, with every seam a removal
/// reaches answered by something a test wrote down.
fn ctx(heard: &Arc<Heard>) -> Ctx {
    running(heard, Arc::new(Scripted(Ok(spoke("")))))
}

/// The same machine, with the programs a removal runs answered by a given runner.
///
/// Apart from [`ctx`] because a case whose claim is about a command that ran cannot
/// make it from what came back: the image removal and the Compose invocation both
/// answer the same way, so which of them was handed over is only in the argument
/// vectors.
fn running(heard: &Arc<Heard>, runner: Arc<dyn Runner>) -> Ctx {
    Ctx::new(
        runner,
        Arc::new(Reporting::holding(
            &["sonarr"],
            Lifecycle::Running,
            Health::Healthy,
        )),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: Arc::new(SeedFs::keyed(None, None).with_facts(StorageFacts {
                point: PathBuf::from("/srv/media"),
                kind: FsKind::Linking("apfs".to_owned()),
                removable: false,
                available: 100,
                total: 1_000,
            })),
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        Settings {
            project: "lemonfiber".to_owned(),
            env_file: Some(PathBuf::from("/cfg/lemonfiber/.env")),
            stack_dir: Some(PathBuf::from("/data/lemonfiber/stack")),
            data_root: Some(PathBuf::from("/srv/media")),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_images(Pulled::holding(Vec::new()))
    .surveying(Walking::holding(Vec::new()))
    .erasing(Erasing::willing())
    .narrating(Arc::clone(heard) as Arc<dyn Narrator>)
    .waiting(Duration::ZERO)
}

/// The envelope a dispatched command renders, or nothing where it refused.
fn rendered(outcome: Result<Outcome, Box<lemonfiber_core::error::Problem>>) -> Option<String> {
    outcome
        .ok()
        .and_then(|outcome| outcome.envelope().to_json())
}

/// The command routes, and what comes back is a document a script can key
/// off — which is what the machine-readable half of every surface reads.
#[tokio::test]
async fn a_reading_is_dispatched_and_renders_as_a_document() {
    let heard = Arc::new(Heard::default());
    let json = rendered(
        dispatch(
            Command::Uninstall(Removing::surveying(Tier::Services)),
            &ctx(&heard),
        )
        .await,
    )
    .unwrap_or_default();

    assert!(json.contains(r#""kind":"uninstall""#), "{json}");
    assert!(json.contains(r#""state":"surveyed""#), "{json}");
    assert!(json.contains(r#""tier":"services""#), "{json}");
}

/// Every one of the four is dispatched, so no removal is reachable on one
/// surface and unreachable through the door all three go through.
#[tokio::test]
async fn every_one_of_the_four_removals_is_dispatched() {
    let heard = Arc::new(Heard::default());
    let ctx = ctx(&heard);

    for tier in lemonfiber_core::uninstall::TIERS {
        let json = rendered(dispatch(Command::Uninstall(Removing::surveying(tier)), &ctx).await)
            .unwrap_or_default();
        assert!(
            json.contains(&format!(r#""tier":"{}""#, tier.name())),
            "{tier:?}: {json}"
        );
    }
}

/// A removal told to wait waits, and says so where the surface is
/// listening rather than reporting it at the end.
#[tokio::test(start_paused = true)]
async fn a_removal_told_to_wait_says_what_it_is_waiting_for() {
    let heard = Arc::new(Heard::default());
    let asked = Removing::surveying(Tier::Stop)
        .confirmed(true)
        .waiting(Waiting::ForTheDownloads);

    let outcome = dispatch(Command::Uninstall(asked), &ctx(&heard)).await;

    assert!(
        matches!(
            outcome,
            Ok(Outcome::Uninstall(ref answered))
                if matches!(answered.removal, Removal::Complete { .. })
        ),
        "the removal did not complete"
    );
    let said = heard.said().await;
    assert!(
        said.iter().any(|line| line.contains("downloads finished")),
        "{said:?}"
    );
}

/// A confirmed removal is carried out from here, and what it could not take comes
/// back named with the way to finish it.
///
/// Driven from outside the crate rather than beside the code, for the reason the
/// wait above is: the removal is `async` all the way down, and an `async` path
/// exercised only in-crate has its coverage counted from a copy that never ran — so
/// the whole of what a removal does would read as reached while one line of it was
/// not.
#[tokio::test]
async fn a_removal_the_platform_refused_comes_back_with_the_way_to_finish_it() {
    let heard = Arc::new(Heard::default());
    let eraser = Erasing::refusing("permission denied");
    let ctx = ctx(&heard).erasing(Arc::clone(&eraser) as Arc<dyn Eraser>);
    let asked = Removing::surveying(Tier::Configuration).confirmed(true);

    let outcome = dispatch(Command::Uninstall(asked), &ctx).await;

    let left = match outcome {
        Ok(Outcome::Uninstall(answered)) => match answered.removal {
            Removal::Partial { left, .. } => left,
            Removal::Surveyed | Removal::Confirmed | Removal::Complete { .. } => Vec::new(),
        },
        Ok(_) | Err(_) => Vec::new(),
    };

    assert_eq!(left.len(), 2, "{left:?}");
    let unhelpful: Vec<&lemonfiber_core::uninstall::Left> = left
        .iter()
        .filter(|one| one.why != "permission denied" || !one.by_hand.contains("owns it"))
        .collect();
    assert!(unhelpful.is_empty(), "{unhelpful:?}");
    assert_eq!(
        eraser.asked(),
        vec![
            PathBuf::from("/cfg/lemonfiber"),
            PathBuf::from("/data/lemonfiber")
        ]
    );
}

/// A confirmed removal of the containers hands the engine the image this stack
/// alone is standing on.
///
/// The fourth tier confirmed through the door: the wait above confirms the first,
/// the refusal above it the third, and the library the fourth. Left out, the one
/// tier that removes images is reached by nothing an operator can type — and the
/// arm that reaches it is a line of the copy of this dispatcher that a `tests/`
/// run compiles, so nothing outside would have counted it as run either.
#[tokio::test]
async fn a_confirmed_removal_of_the_containers_hands_the_image_to_the_engine() {
    let heard = Arc::new(Heard::default());
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx =
        running(&heard, Arc::clone(&runner) as Arc<dyn Runner>).with_images(Pulled::holding(vec![
            Pulled::image(SONARR, 400, &["lemonfiber"]),
        ]));
    let asked = Removing::surveying(Tier::Services).confirmed(true);

    let outcome = dispatch(Command::Uninstall(asked), &ctx).await;

    let gone = match outcome {
        Ok(Outcome::Uninstall(answered)) => match answered.removal {
            Removal::Complete { gone, .. } => gone,
            Removal::Surveyed | Removal::Confirmed | Removal::Partial { .. } => Vec::new(),
        },
        Ok(_) | Err(_) => Vec::new(),
    };

    assert!(gone.iter().any(|name| name == SONARR), "{gone:?}");
    let about_the_image: Vec<Vec<String>> = runner
        .seen()
        .into_iter()
        .filter(|argv| argv.iter().any(|word| word == SONARR))
        .collect();
    assert_eq!(
        about_the_image,
        vec![vec![
            "docker".to_owned(),
            "image".to_owned(),
            "rm".to_owned(),
            SONARR.to_owned(),
        ]]
    );
}

/// The removal that reaches the library is refused from here too, so the
/// rule is the command's rather than one surface's.
#[tokio::test]
async fn the_removal_that_reaches_the_library_is_refused_without_its_own_answer() {
    let heard = Arc::new(Heard::default());
    let asked = Removing::surveying(Tier::Media).confirmed(true);

    let refused = dispatch(Command::Uninstall(asked), &ctx(&heard)).await;

    assert!(
        refused.is_err_and(|problem| problem.code == lemonfiber_core::uninstall::NEEDS_AGREEING)
    );
}
