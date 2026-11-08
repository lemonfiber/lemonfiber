//! A job's check-in, made at each beat it acts on, to the heartbeat lemonfiber provisioned
//! for it.

use crate::app::Ctx;

use super::covering::Job;

/// What checking in came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckIn {
    /// Nothing to check in to: a rehearsal, no heartbeat provisioned for the job, or no
    /// watcher to ask.
    Unwatched,
    /// The watcher took it.
    Taken,
    /// The watcher did not take it, which its heartbeat says as a missed check-in.
    Missed,
}

/// Check `job` in to its heartbeat.
///
/// Asked only where the record holds a heartbeat for the job, so a job nobody hosts asks
/// nothing of the watcher.
pub(crate) async fn checked_in(ctx: &Ctx, job: Job) -> CheckIn {
    if ctx.dry_run {
        return CheckIn::Unwatched;
    }
    let tag = job.covers().tag();
    if !super::kept::held(ctx).made.contains_key(&tag) {
        return CheckIn::Unwatched;
    }
    let Some((_, _, fillers)) = crate::seed::run::managed(ctx) else {
        return CheckIn::Unwatched;
    };
    let Some(watcher) = super::asked(ctx, &fillers).await else {
        return CheckIn::Unwatched;
    };
    match watcher.check_in(&tag).await {
        Ok(()) => CheckIn::Taken,
        Err(_) => CheckIn::Missed,
    }
}

#[cfg(test)]
mod tests;
