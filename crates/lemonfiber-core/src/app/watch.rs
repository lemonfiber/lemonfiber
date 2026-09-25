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
use crate::error::codes::watch::{ALREADY_GONE, NOTHING_TO_WATCH};

/// How often a watch re-checks that the data root is still there.
///
/// Frequent enough to stop the services before much is written into a vanished
/// mount, and no more, because the check is a stat and doing it in a tight loop
/// would spin a core to catch an event that arrives in seconds at worst.
pub const WATCH: Duration = Duration::from_secs(5);

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
mod tests;
