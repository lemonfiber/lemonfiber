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

use crate::error::{Problem, Remedy, Severity, State};
use crate::ports::filesystem::Presence;
use crate::stack::compose::Action;

use super::super::Ctx;
use crate::error::codes::life::NO_DATA_LOCATION;

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
    !matches!(ctx.seams.volume.presence(root).await, Presence::Gone)
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
mod tests;
