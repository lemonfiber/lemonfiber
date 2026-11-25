//! The dashboard a surface gathers: the last gather, held for the run, and the wake-ups
//! and deliveries a new alert owes.

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::alert::Alert;
use crate::dashboard::run::{waves, Gathered};
use crate::dashboard::Snapshot;
use crate::notify::fanning::Fanning;
use crate::push::run::Waking;

use super::Ctx;

/// Where the fast panels of a gather are handed before its slow ones are in.
pub trait Early: Send + Sync {
    /// Hand on the first wave of a gather.
    fn first(&self, snapshot: Snapshot);
}

/// Where nobody waits for a first wave.
pub(crate) struct Unheard;

impl Early for Unheard {
    fn first(&self, _snapshot: Snapshot) {}
}

/// What a run holds of its dashboard: the last gather, and the deliveries under way.
#[derive(Default)]
pub struct Dashboarding {
    last: Mutex<Option<Gathered>>,
    waking: Waking,
    fanning: Fanning,
}

impl Dashboarding {
    fn owing(&self, ctx: &Ctx, alerts: &[Alert]) {
        let run = Arc::new(ctx.clone());
        self.waking.owing(&run, alerts);
        self.fanning.owing(&run, alerts);
    }
}

/// Whether a gather owes the wake-ups and deliveries a new alert asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Owes {
    /// The operator's own gather, on the run serving the web surface.
    Alerts,
    /// A look at the dashboard that owes nobody anything.
    Nothing,
}

pub(super) async fn gathered(ctx: &Ctx, now: bool, owes: Owes) -> Snapshot {
    let held = &ctx.dashboarding;
    let mut last = held.last.lock().await;
    let due = last.as_ref().filter(|_| now).map(Gathered::due_now);
    let previous = if now { due.as_ref() } else { last.as_ref() };
    let early = Arc::clone(&ctx.early);
    let fresh = waves(ctx, previous, move |first| early.first(first)).await;
    let snapshot = fresh.snapshot.clone();
    *last = Some(fresh);
    drop(last);
    if owes == Owes::Alerts && !ctx.dry_run {
        held.owing(ctx, &snapshot.alerts);
    }
    snapshot
}

#[cfg(test)]
mod tests;
