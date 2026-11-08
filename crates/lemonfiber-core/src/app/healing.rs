//! Queue self-healing: every queued download classified from what its curator and its
//! download client each say of it, and each one flagged for the same reason on enough
//! consecutive passes dropped by its curator, its release blocklisted, and its title
//! searched for again.
//!
//! It is the one way lemonfiber acts on a wedged download, and it acts only where the
//! operator has switched it on; off, the queue is still classified and read. Nothing is
//! acted on for a curator whose download client it cannot reach, or whose client files
//! its downloads outside the curator's folder.

use crate::app::Ctx;
use crate::config::Healing;
use crate::error::Problem;
use crate::model::QueueHealth;

mod acting;
mod gathering;
mod judging;
pub(crate) mod kept;

/// How the queue heals, as the operator recorded it.
pub(crate) fn healing(ctx: &Ctx) -> Healing {
    ctx.settings
        .env_file
        .as_deref()
        .and_then(|path| crate::config::store::read(path).ok())
        .map(|file| Healing::from_env(&file))
        .unwrap_or_default()
}

/// One pass at the guard's pace: nothing where its own pace has not come round, and
/// otherwise the queue classified, and acted on where healing is on, a sentence each.
pub(crate) async fn paced(ctx: &Ctx) {
    if ctx.dry_run {
        return;
    }
    let healing = healing(ctx);
    let held = kept::held(ctx);
    let now = ctx.seconds();
    let due = held
        .latest
        .as_ref()
        .is_none_or(|latest| now.saturating_sub(latest.at) >= healing.every.as_secs());
    if due {
        told(ctx, pass(ctx, healing, held).await).await;
    }
}

/// One pass now, as `lemonfiber queue heal` asks for it: acted on where healing is on, and
/// rehearsed where the run only says what it would do; each act said in a sentence.
///
/// # Errors
///
/// Where the record of the queue cannot be read, which is never taken as a queue nobody
/// has looked at.
pub(crate) async fn heal(ctx: &Ctx) -> Result<QueueHealth, Box<Problem>> {
    let held = kept::read(ctx)?;
    let healing = healing(ctx);
    told(ctx, pass(ctx, healing, held).await).await;
    let mut health = queue(ctx)?;
    health.rehearsed = ctx.dry_run;
    Ok(health)
}

/// Say each of `said` to the operator.
async fn told(ctx: &Ctx, said: Vec<String>) {
    for one in said {
        ctx.narrator.say(&one).await;
    }
}

/// The queue as the latest pass left it, as `lemonfiber queue` reads it.
///
/// # Errors
///
/// Where the record of the queue cannot be read, which is never answered as a healthy
/// queue.
pub(crate) fn queue(ctx: &Ctx) -> Result<QueueHealth, Box<Problem>> {
    let held = kept::read(ctx)?;
    Ok(held.read(&healing(ctx)))
}

/// Classify the queue, strike what is wedged, act on what has struck out where healing is
/// on or the run is a rehearsal, and keep what it came to; each act said in a sentence.
///
/// A rehearsal keeps its run and what it found, and advances no strike.
async fn pass(ctx: &Ctx, healing: Healing, mut held: kept::Kept) -> Vec<String> {
    let now = ctx.seconds();
    let gathered = gathering::gathered(ctx).await;
    let judged = judging::judged(&gathered, &held.strikes, &healing, now);
    let mut run = Vec::new();
    let mut acted = Vec::new();
    let mut said = Vec::new();
    if healing.on || ctx.dry_run {
        for due in judged.due(&healing, now) {
            let healed = acting::acted(ctx, &gathered, due).await;
            said.push(acting::said(&healed, ctx.dry_run));
            if acting::took(&healed) {
                acted.push(due.key.clone());
            }
            run.push(healed);
        }
    }
    held.passed(&judged, &gathered, &healing, now, &acted, ctx.dry_run);
    if !run.is_empty() {
        held.ran(crate::model::HealRun {
            at: now,
            rehearsed: ctx.dry_run,
            items: run,
        });
    }
    kept::keep(ctx, &held);
    said
}

#[cfg(test)]
mod tests;
