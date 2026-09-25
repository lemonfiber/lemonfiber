use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use super::{assess, supervise, Availability, Loss, ALREADY_GONE, NOTHING_TO_WATCH};
use crate::app::Ctx;
use crate::config::{Protocols, Settings};
use crate::error::Problem;
use crate::model::SupervisionReport;
use crate::ports::filesystem::{Presence, Volume};
use crate::ports::process::{Failure, Output};
use crate::test_support::{a_context, spoke, Reporting, Scripted};

/// A volume that answers each check with the next reading a test scripted,
/// then stays gone once the script runs out.
struct Drive {
    readings: Vec<Presence>,
    cursor: AtomicUsize,
}

impl Drive {
    fn playing(readings: Vec<Presence>) -> Self {
        Self {
            readings,
            cursor: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Volume for Drive {
    async fn presence(&self, _path: &Path) -> Presence {
        let at = self.cursor.fetch_add(1, Ordering::Relaxed);
        self.readings.get(at).copied().unwrap_or(Presence::Gone)
    }
}

/// A context whose runner answers the stop with `result`, watching the given
/// data location.
fn watching(result: Result<Output, Failure>, data_root: Option<&str>) -> Ctx {
    let settings = Settings {
        protocols: Protocols::both(),
        data_root: data_root.map(std::path::PathBuf::from),
        ..Settings::default()
    };
    a_context()
        .runner(Arc::new(Scripted(result)))
        .engine(Arc::new(Reporting::default()))
        .settings(settings)
        .build()
}

async fn watch(ctx: &Ctx, drive: Drive) -> Result<SupervisionReport, Box<Problem>> {
    supervise(
        ctx,
        &drive,
        &["library".to_owned()],
        std::time::Duration::ZERO,
    )
    .await
}

#[test]
fn a_reading_is_judged_against_the_volume_the_watch_started_on() {
    assert!(matches!(
        assess(9, Presence::Gone),
        Availability::Lost(Loss::Vanished)
    ));
    assert!(matches!(assess(9, Presence::On(9)), Availability::Holding));
    assert!(matches!(
        assess(9, Presence::On(4)),
        Availability::Lost(Loss::Moved)
    ));
    // A reading that could not be taken is held, not treated as a loss.
    assert!(matches!(
        assess(9, Presence::Unknown),
        Availability::Holding
    ));
}

#[tokio::test]
async fn a_watch_with_no_data_location_says_there_is_nothing_to_watch() {
    let ctx = watching(Ok(spoke("")), None);
    let refused = watch(&ctx, Drive::playing(vec![])).await.err();
    assert_eq!(refused.map(|problem| problem.code), Some(NOTHING_TO_WATCH));
}

#[tokio::test]
async fn a_location_already_gone_when_the_watch_begins_will_not_start() {
    let ctx = watching(Ok(spoke("")), Some("/data"));
    let refused = watch(&ctx, Drive::playing(vec![Presence::Gone]))
        .await
        .err();
    assert_eq!(refused.map(|problem| problem.code), Some(ALREADY_GONE));
}

#[tokio::test]
async fn a_data_root_that_vanishes_stops_the_services() {
    let ctx = watching(Ok(spoke("")), Some("/data"));
    let report = watch(&ctx, Drive::playing(vec![Presence::On(9), Presence::Gone]))
        .await
        .ok();
    assert_eq!(
        report.map(|report| (report.stopped, report.reason.contains("no longer present"))),
        Some((true, true))
    );
}

#[tokio::test]
async fn a_data_root_that_holds_before_it_is_lost_keeps_checking() {
    let ctx = watching(Ok(spoke("")), Some("/data"));
    let report = watch(
        &ctx,
        Drive::playing(vec![Presence::On(9), Presence::On(9), Presence::Gone]),
    )
    .await
    .ok();
    assert_eq!(report.map(|report| report.stopped), Some(true));
}

#[tokio::test(start_paused = true)]
async fn a_watch_asked_for_as_a_command_guards_the_volume_the_context_names() {
    // The whole of what a surface has to supply: which drive to ask about is
    // the context's, and how often to ask is this command's own — a surface
    // that could choose the interval could choose one that misses the moment
    // the watch exists for.
    let ctx = watching(Ok(spoke("")), Some("/data")).with_volume(Arc::new(Drive::playing(vec![
        Presence::On(9),
        Presence::Gone,
    ])));
    let outcome = crate::app::dispatch(
        crate::app::Command::Watch {
            forms: vec!["library".to_owned()],
        },
        &ctx,
    )
    .await;

    // The same report `supervise` comes to when it is asked directly, so
    // dispatching changes only which drive it asks and how often.
    let directly = watch(
        &watching(Ok(spoke("")), Some("/data")),
        Drive::playing(vec![Presence::On(9), Presence::Gone]),
    )
    .await;
    assert_eq!(outcome.ok(), directly.ok().map(crate::app::Outcome::Watch));
}

#[tokio::test]
async fn a_watch_asked_for_with_nothing_to_guard_is_refused_as_a_command_too() {
    let ctx = watching(Ok(spoke("")), None);
    let refused = crate::app::dispatch(
        crate::app::Command::Watch {
            forms: vec!["library".to_owned()],
        },
        &ctx,
    )
    .await
    .err();
    assert_eq!(refused.map(|problem| problem.code), Some(NOTHING_TO_WATCH));
}

#[tokio::test]
async fn a_reading_that_cannot_be_taken_is_held_not_acted_on() {
    // A transient error mid-watch — the drive is still there. The watch holds
    // through it and only stops when the volume genuinely goes.
    let ctx = watching(Ok(spoke("")), Some("/data"));
    let report = watch(
        &ctx,
        Drive::playing(vec![Presence::On(9), Presence::Unknown, Presence::Gone]),
    )
    .await
    .ok();
    assert_eq!(
        report.map(|report| (report.stopped, report.reason.contains("no longer present"))),
        Some((true, true)),
        "the hiccup was held; the real loss stopped the services"
    );
}

#[tokio::test]
async fn a_rehearsed_watch_says_what_it_would_guard_and_never_takes_a_second_look() {
    // The one command where stopping short of the last step would mean not
    // stopping at all: a watch is a wait, so a rehearsal that ran it would hold
    // until somebody unplugged the drive.
    let ctx = watching(Ok(spoke("")), Some("/data")).rehearsing();
    let drive = Drive::playing(vec![Presence::On(9), Presence::Gone]);
    let report = supervise(
        &ctx,
        &drive,
        &["library".to_owned()],
        std::time::Duration::from_secs(5),
    )
    .await
    .ok();

    assert_eq!(
        drive.cursor.load(Ordering::Relaxed),
        1,
        "it looked twice, which is watching"
    );
    let would = report.and_then(|report| report.would);
    assert_eq!(
        would
            .as_ref()
            .map(|would| (would.root.clone(), would.every)),
        Some(("/data".to_owned(), 5))
    );
    assert!(
        would.is_some_and(|would| would.command.iter().any(|word| word == "stop")),
        "a rehearsal that printed no invocation, or one that was not the stop, \
         printed nothing worth reading"
    );
}

#[tokio::test]
async fn a_rehearsed_watch_with_nothing_to_guard_is_refused_the_way_a_real_one_is() {
    // The gates are facts about the machine rather than consequences of acting, so
    // a rehearsal meets them where a real watch meets them.
    let ctx = watching(Ok(spoke("")), None).rehearsing();
    let refused = watch(&ctx, Drive::playing(vec![])).await.err();
    assert_eq!(refused.map(|problem| problem.code), Some(NOTHING_TO_WATCH));
}

#[tokio::test]
async fn a_data_root_that_becomes_a_different_volume_is_a_loss() {
    let ctx = watching(Ok(spoke("")), Some("/data"));
    let report = watch(&ctx, Drive::playing(vec![Presence::On(9), Presence::On(4)]))
        .await
        .ok();
    assert_eq!(
        report.map(|report| report.reason.contains("different volume")),
        Some(true)
    );
}

#[tokio::test]
async fn services_that_cannot_be_stopped_are_reported_not_hidden() {
    let ctx = watching(
        Err(Failure::NotFound {
            program: "docker".to_owned(),
        }),
        Some("/data"),
    );
    let report = watch(&ctx, Drive::playing(vec![Presence::On(9), Presence::Gone]))
        .await
        .ok();
    assert_eq!(report.map(|report| report.stopped), Some(false));
}
