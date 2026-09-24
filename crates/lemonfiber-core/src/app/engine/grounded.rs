//! Proving the data location is there before anything is started over it.
//!
//! An external drive or a network share is not mounted the instant a machine is
//! ready to run things, and Compose does not care: a bind mount whose source is
//! missing is a directory the engine creates, on whatever filesystem the mount point
//! happens to sit on — which is usually the system disk. The stack then starts
//! perfectly, files a library into it, and reports nothing wrong at all, while the
//! operator's real library is offline and their boot volume fills with a copy of it
//! that is orphaned the moment the drive comes back.
//!
//! The guard that watches for this *while the stack runs* has existed for a while
//! and was never asked before one started, so the one moment it could not help was
//! the moment a machine has just been switched on.
//!
//! **It waits before it refuses.** A drive three seconds behind the login and a
//! drive that is not plugged in at all look identical at the instant a start is asked
//! for, and refusing on the first reading would make every restart of a machine with
//! an external library a coin toss. So an absent location is looked for again, on the
//! interval the running guard uses, and only one that never turns up is reported.
//!
//! A reading that could not be taken at all is not an absence. A permission error or
//! an interrupted call says nothing about whether the drive is there, and treating it
//! as a loss would stop a stack over a hiccup — the same judgement the running guard
//! makes, for the same reason.

use std::path::Path;
use std::time::Duration;

use crate::error::{Code, Problem, Remedy, Severity, State};
use crate::ports::filesystem::Presence;
use crate::stack::compose::Action;

use super::super::Ctx;

/// How many times a start looks again for a location that is not there yet.
///
/// Counted rather than clocked, unlike the settle beside it, because what is being
/// waited on is not a stack this process started: there is no deadline to measure
/// against, only a number of looks worth taking before an absence is an absence.
/// Sixty of them at the interval below is two minutes — long enough for a drive that
/// spins up at login or a share still negotiating, and short enough that a start
/// against a location nobody is going to plug in reports rather than hangs.
const LOOKS: u32 = 60;

/// How long it leaves between two looks.
///
/// The interval the running guard uses, for the reason that one does: the check is a
/// stat, and taking it in a tight loop would spin a core to catch an event that
/// arrives in seconds at worst.
const AGAIN: Duration = Duration::from_secs(2);

/// Raised when a start was asked for over a data location that is not there.
pub const NO_DATA_LOCATION: Code = Code::new("LIFE-5");

/// Refuse to start over a data location that is not present, waiting first.
///
/// Only a start is held to it. Stopping, restarting and fetching either take nothing
/// away from the disk or need no disk at all, and a teardown blocked because a drive
/// had been unplugged would refuse to tidy up after exactly the accident it is being
/// run about.
///
/// A machine with no data location configured is held to nothing either: there is
/// nothing to prove present, and where one should be is setup's question.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render where the data location never
/// appeared.
pub(crate) async fn grounded(ctx: &Ctx, action: &Action) -> Result<(), Box<Problem>> {
    waited(ctx, action, LOOKS, AGAIN).await
}

/// The same thing with the waiting spelled out, so a test can drive it without
/// sitting through two minutes of a real clock.
///
/// The budget is a count and an interval rather than a deadline for the reason
/// [`LOOKS`] gives, and both are arguments here for the reason the running guard's
/// interval is one: a wait that could only be exercised at its real length is a wait
/// nobody exercises.
async fn waited(
    ctx: &Ctx,
    action: &Action,
    looks: u32,
    again: Duration,
) -> Result<(), Box<Problem>> {
    if !super::starts(action) {
        return Ok(());
    }
    let Some(root) = ctx.settings.data_root.as_deref() else {
        return Ok(());
    };
    if present(ctx, root).await {
        return Ok(());
    }
    ctx.narrator
        .say(&format!(
            "waiting for the data location {} to appear",
            root.display()
        ))
        .await;
    for _ in 0..looks {
        tokio::time::sleep(again).await;
        if present(ctx, root).await {
            return Ok(());
        }
    }
    Err(Box::new(never_appeared(root)))
}

/// Whether the location is there now, or at least readable enough not to be an
/// absence.
async fn present(ctx: &Ctx, root: &Path) -> bool {
    !matches!(ctx.volume.presence(root).await, Presence::Gone)
}

/// What to tell an operator whose data location never turned up.
///
/// It names what starting anyway would have done rather than only what did not
/// happen: an operator told a start was refused wants to know why that is the
/// kindness rather than the obstruction, and "a second library on the system disk" is
/// the sentence that says so.
fn never_appeared(root: &Path) -> Problem {
    Problem::new(
        NO_DATA_LOCATION,
        Severity::Error,
        format!("The data location {} is not there", root.display()),
        "Nothing was started. Starting over a location that is not mounted does not fail — the \
         engine makes the directory on whatever is underneath the mount point, which is usually \
         the system disk, and the stack then files a second library into it while the real one \
         is offline.",
        Remedy::new("Connect the drive or mount holding the data location, then start again")
            .with_detail(
                "If the location has moved for good, run setup and choose where it is now",
            ),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;

    use super::{grounded, waited, NO_DATA_LOCATION};
    use crate::app::Ctx;
    use crate::config::Settings;
    use crate::ports::filesystem::{Presence, Volume};
    use crate::stack::compose::Action;
    use crate::test_support::a_context;

    /// A drive answering each reading with the next one a test scripted, then staying
    /// on the last one once the script runs out.
    struct Drive {
        readings: Vec<Presence>,
        cursor: AtomicUsize,
    }

    #[async_trait]
    impl Volume for Drive {
        async fn presence(&self, _path: &Path) -> Presence {
            let at = self.cursor.fetch_add(1, Ordering::SeqCst);
            self.readings
                .get(at)
                .or_else(|| self.readings.last())
                .copied()
                .unwrap_or(Presence::Gone)
        }
    }

    /// A context whose data location answers with the given readings in turn.
    fn ctx_reading(readings: Vec<Presence>) -> Ctx {
        a_context()
            .settings(Settings {
                data_root: Some(PathBuf::from("/Volumes/media")),
                ..Settings::default()
            })
            .build()
            .with_volume(Arc::new(Drive {
                readings,
                cursor: AtomicUsize::new(0),
            }))
    }

    /// The wait, driven with a handful of looks and no interval at all.
    async fn over(ctx: &Ctx, action: &Action) -> Result<(), Box<crate::error::Problem>> {
        waited(ctx, action, 3, Duration::ZERO).await
    }

    #[tokio::test]
    async fn a_location_that_is_there_holds_nothing_up() {
        let ctx = ctx_reading(vec![Presence::On(7)]);
        assert!(grounded(&ctx, &Action::Up).await.is_ok());
    }

    #[tokio::test]
    async fn a_location_that_turns_up_late_is_waited_for_rather_than_refused() {
        // The drive that spins up a few seconds after the login. Refusing on the
        // first reading would make every restart of a machine with an external
        // library a coin toss.
        let ctx = ctx_reading(vec![Presence::Gone, Presence::Gone, Presence::On(7)]);
        assert!(over(&ctx, &Action::Up).await.is_ok());
    }

    #[tokio::test]
    async fn a_location_that_never_appears_stops_the_start_and_says_why() {
        // The whole point: the engine would otherwise make the directory on the
        // system disk and file a library into it while the real one is offline.
        let ctx = ctx_reading(vec![Presence::Gone]);

        let refused = over(&ctx, &Action::Up).await.err();

        assert_eq!(
            refused.as_ref().map(|problem| problem.code),
            Some(NO_DATA_LOCATION)
        );
        assert!(
            refused
                .as_ref()
                .is_some_and(|problem| problem.summary.contains("/Volumes/media")),
            "it names the location that is missing"
        );
        assert!(
            refused.is_some_and(|problem| problem.meaning.contains("system disk")),
            "and what starting anyway would have done"
        );
    }

    #[tokio::test]
    async fn a_reading_that_could_not_be_taken_is_not_an_absence() {
        // A permission error or an interrupted call says nothing about whether the
        // drive is there, and stopping a start over one would be stopping it over a
        // hiccup — the judgement the running guard makes, for the same reason.
        let ctx = ctx_reading(vec![Presence::Unknown]);
        assert!(grounded(&ctx, &Action::Up).await.is_ok());
    }

    #[tokio::test]
    async fn nothing_but_a_start_is_held_to_it() {
        // A teardown blocked because a drive had been unplugged would refuse to tidy
        // up after exactly the accident it is being run about.
        let ctx = ctx_reading(vec![Presence::Gone]);
        for action in [
            Action::Down,
            Action::Stop(Vec::new()),
            Action::Restart(Vec::new()),
            Action::Pull,
            Action::Config,
        ] {
            assert!(
                over(&ctx, &action).await.is_ok(),
                "{action:?} does not write into the data location"
            );
        }
    }

    #[tokio::test]
    async fn starting_named_services_is_held_to_it_too() {
        // They write into the same location the whole form does, so the narrower
        // request is the same hazard with fewer containers in it.
        let ctx = ctx_reading(vec![Presence::Gone]);
        assert!(over(&ctx, &Action::Start(vec!["sonarr".to_owned()]))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn a_machine_with_no_data_location_has_nothing_to_prove() {
        let ctx = a_context()
            .settings(Settings::default())
            .build()
            .with_volume(Arc::new(Drive {
                readings: vec![Presence::Gone],
                cursor: AtomicUsize::new(0),
            }));
        assert!(over(&ctx, &Action::Up).await.is_ok());
    }
}
