//! Guarding the data location while the stack runs over it.
//!
//! A watch stats the data root on an interval and, the moment it is gone or has
//! become a different volume — a drive unplugged from under a surviving mount
//! point — stops the services rather than letting them write a phantom library
//! onto whatever is left. It never restarts them: whether the state that came
//! back is trustworthy is the operator's call, not this one's.
//!
//! Split out of the dispatcher for cohesion — the verbs each already live in
//! their own `app` submodule, and the watch is a self-contained feature with its
//! own codes, its own loss taxonomy, and one entry point (`supervise`).

use std::path::Path;
use std::time::Duration;

use crate::error::{Problem, Remedy, Severity, State};
use crate::model::{SupervisionReport, Vigil};
use crate::ports::filesystem::{Presence, Volume};
use crate::stack::compose::Action;

use super::engine::{invocation, lifecycle};
use super::{Ctx, Outcome};

/// How often a watch re-checks that the data root is still there.
///
/// Frequent enough to stop the services before much is written into a vanished
/// mount, and no more, because the check is a stat and doing it in a tight loop
/// would spin a core to catch an event that arrives in seconds at worst.
pub const WATCH: Duration = Duration::from_secs(5);

pub use crate::error::codes::watch::NOTHING_TO_WATCH;

pub use crate::error::codes::watch::ALREADY_GONE;

/// What a run that only said what a watch would do puts where the ending goes.
///
/// Said rather than left empty, because the field is read by a caller that has no
/// reason to look at `would` first, and a blank reason reads as a watch that ended
/// for no reason anybody wrote down.
const NOTHING_WAS_WATCHED: &str = "nothing was watched: this run said what a watch \
     would do and kept none";

/// How a watch ended.
enum Loss {
    /// The data root's path is no longer there at all.
    Vanished,
    /// The path is there, but on a different volume than it started on — the
    /// shape of a drive pulled out from under a surviving mount point.
    Moved,
}

/// Whether the data root is still the one the watch began guarding.
enum Availability {
    /// Present, and the same volume as before.
    Holding,
    /// Lost, and how.
    Lost(Loss),
}

/// Read a fresh presence against the one the watch started with.
///
/// A path on a different volume is a loss, not a presence: it is what a mount
/// point left behind by an unplugged drive looks like, and treating it as "still
/// there" is exactly the mistake that lets the services write a phantom library
/// onto the system disk.
fn assess(baseline: u64, current: Presence) -> Availability {
    match current {
        Presence::Gone => Availability::Lost(Loss::Vanished),
        Presence::On(volume) if volume == baseline => Availability::Holding,
        Presence::On(_) => Availability::Lost(Loss::Moved),
        // A reading that could not be taken is not a loss. A permission error or
        // an interrupted stat says nothing about whether the drive is still
        // there, so the watch holds and lets a later poll settle it rather than
        // stopping the stack on a hiccup.
        Presence::Unknown => Availability::Holding,
    }
}

/// Poll the data root until it is lost, and say how it was lost.
async fn watch_until_lost(
    volume: &dyn Volume,
    root: &Path,
    baseline: u64,
    interval: Duration,
) -> Loss {
    loop {
        tokio::time::sleep(interval).await;
        match assess(baseline, volume.presence(root).await) {
            Availability::Holding => {}
            Availability::Lost(loss) => return loss,
        }
    }
}

/// Watch the data root while the given forms run, and stop them the moment it is
/// lost — never restarting it, because whether the state that came back is
/// trustworthy is the operator's call, not this one's.
///
/// # Errors
///
/// Returns a [`Problem`] where there is no data location configured to watch, or
/// where it is already gone before the watch can begin.
pub async fn supervise(
    ctx: &Ctx,
    volume: &dyn Volume,
    forms: &[String],
    interval: Duration,
) -> Result<SupervisionReport, Box<Problem>> {
    let Some(root) = ctx.settings.data_root.as_deref() else {
        return Err(Box::new(nothing_to_watch()));
    };
    let baseline = match volume.presence(root).await {
        Presence::On(volume) => volume,
        // Missing, or unreadable at the outset: either way there is no baseline
        // to watch against, so the watch does not begin. Once it is running, an
        // unreadable reading is held rather than acted on — see `assess`.
        Presence::Gone | Presence::Unknown => return Err(Box::new(already_gone(root))),
    };

    // A rehearsal stops here, and it is the one command where that is not the same
    // shape as everywhere else: the step this leaves out is not the last one but all
    // of them, because a watch is a wait. Running it as a rehearsal would hold the
    // terminal until somebody unplugged the drive, which is a worse answer than the
    // refusal it replaces. What it reports instead is the watch itself — where it
    // would look, how often, and the invocation it would run the moment that location
    // went — and both gates above still apply, because a location that is not there
    // is not one a watch would have held either.
    if ctx.dry_run {
        return would_watch(ctx, root, forms, interval);
    }

    let loss = watch_until_lost(volume, root, baseline, interval).await;

    // The stop is attempted whatever its outcome: the data root is gone either
    // way, and reporting that the services could not be stopped is more use than
    // refusing to report the loss at all.
    let stopped = matches!(
        lifecycle(ctx, forms, &Action::Stop(Vec::new())).await,
        Ok(Outcome::Lifecycle(report)) if report.status == Some(0)
    );

    Ok(SupervisionReport {
        forms: forms.to_vec(),
        reason: describe_loss(&loss),
        stopped,
        would: None,
    })
}

/// The watch this run would have kept, reported in place of keeping it.
///
/// The invocation comes from the same prelude a real stop is built by, rather than
/// from a sentence written here that says what that prelude produces. Two accounts of
/// one argv is one account nobody runs, and the one nobody runs is the one that stops
/// being true — which on this command would not be found out until a drive was pulled.
///
/// # Errors
///
/// Returns the [`Problem`] the stop itself would give where the stack cannot be read,
/// resolved or written — met here, before the wait, rather than at the end of one.
fn would_watch(
    ctx: &Ctx,
    root: &Path,
    forms: &[String],
    interval: Duration,
) -> Result<SupervisionReport, Box<Problem>> {
    let (command, _) = invocation(ctx, forms, &Action::Stop(Vec::new()))?;
    Ok(SupervisionReport {
        forms: forms.to_vec(),
        reason: NOTHING_WAS_WATCHED.to_owned(),
        stopped: false,
        would: Some(Vigil {
            root: root.display().to_string(),
            every: interval.as_secs(),
            command,
        }),
    })
}

/// The one-line reason a watch ended, for the operator.
fn describe_loss(loss: &Loss) -> String {
    match loss {
        Loss::Vanished => "the data location is no longer present".to_owned(),
        Loss::Moved => "the data location is now a different volume — the drive holding it was \
                        most likely disconnected"
            .to_owned(),
    }
}

/// The problem for a watch with no data location to guard.
fn nothing_to_watch() -> Problem {
    Problem::new(
        NOTHING_TO_WATCH,
        Severity::Error,
        "There is no data location to watch",
        "A watch guards the directory your downloads and library live in, and none is configured \
         yet, so there is nothing for it to guard.",
        Remedy::new("Run setup to choose a data location, then start the watch again"),
    )
    .in_state(State::Guided)
}

/// The problem for a watch whose data location is gone before it starts.
fn already_gone(root: &Path) -> Problem {
    Problem::new(
        ALREADY_GONE,
        Severity::Error,
        format!("The data location {} is not available", root.display()),
        "A watch can only guard a location that is present when it begins; this one is already \
         gone, so there is nothing running over it to protect.",
        Remedy::new("Connect the drive or mount holding the data location, then start the watch"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests {
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
        let ctx =
            watching(Ok(spoke("")), Some("/data")).with_volume(Arc::new(Drive::playing(vec![
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
}
