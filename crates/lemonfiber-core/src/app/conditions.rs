//! What is wrong, kept between runs.
//!
//! Everything the conditions are for needs a memory older than one process. How
//! long something has been broken, whether the operator has already been told,
//! whether a fault is steady or flapping — each is a comparison against a previous
//! run, and a store that started empty every time would answer all three wrongly:
//! nothing has lasted any time, everything is news, and nothing ever flaps.
//!
//! Kept with configuration rather than beside the stack, for the reason the
//! baseline is: it is a memory of what was observed, and a backup that restored
//! the stack without it would report a week-old fault as having just started.
//!
//! Best-effort, both ways. A store that cannot be read is an empty one — worse
//! answers for a run, never a refusal to run — and a store that cannot be written
//! costs the next run its history and nothing else. Neither is worth failing a
//! command over.

use crate::condition::{Conditions, Fault};
use crate::doctor::{Finding, Verdict};

use super::Ctx;

/// Read what the last run left, or an empty store where there is none.
///
/// A store that will not parse is treated as absent rather than reported. Unlike
/// the seeding baseline — where a lost record means an operator's edit could be
/// silently overwritten — the worst a lost condition store costs is that standing
/// faults read as new, which the next run corrects on its own.
#[must_use]
pub fn load(ctx: &Ctx) -> Conditions {
    super::record::kept(path(ctx).as_deref())
}

/// Write what this run changed in the store where the next run will read it.
///
/// Laid over the store as it stands rather than written whole, so a run that held
/// the store for a while leaves what another run wrote meanwhile as it found it.
pub fn save(ctx: &Ctx, conditions: &Conditions) {
    // Best effort on the way out, unlike the records an operator decided: a
    // refresh that could not write its history is one the next refresh starts
    // afresh from, which is a worse picture rather than a wrong claim.
    let _ = super::record::keep(path(ctx).as_deref(), &conditions.over(load(ctx)));
}

/// Fold what this run found into the store, and answer with it.
///
/// A diagnosis and a repair both fold through here: the same folding the dashboard
/// does for services, for the same reason. How long a fault has stood, whether it
/// flaps, whether a fix was declined and how often one has failed are all comparisons
/// against previous runs, and none of them can be made by a store that has never heard
/// of the check.
pub(crate) fn remembered(ctx: &Ctx, found: &[Finding]) -> Conditions {
    let mut conditions = load(ctx);
    let now = ctx.stamp();
    for finding in found {
        conditions.observe(&finding.check, wrong(finding).as_ref(), &now);
    }
    // Written down only by a run that is really happening. What this file holds is how
    // often a fault has been seen and how often a fix for it was tried and left it
    // standing, which is how the offer decides what is worth offering again — and a
    // rehearsal that recorded a sighting would move that count without anybody having
    // asked it to. The reading above still happens, because the report a rehearsal
    // gives is built from it.
    if !ctx.dry_run {
        save(ctx, &conditions);
    }
    conditions
}

/// What a finding is remembered as, where it says something is wrong.
///
/// A pass says nothing is wrong and a skip says there was nothing to look at, so neither
/// raises anything. Unverified is the careful one: it means the check could not be
/// established, which is not the same as finding it broken — claiming a fault from it would
/// have lemonfiber remember trouble it never actually saw.
pub(crate) fn wrong(finding: &Finding) -> Option<Fault> {
    let problem = match &finding.verdict {
        Verdict::Warn(problem) | Verdict::Fail(problem) => problem,
        Verdict::Pass { .. } | Verdict::Skipped { .. } | Verdict::Unverified { .. } => return None,
    };
    Some(Fault::new(
        problem.code.as_str(),
        problem.severity,
        &problem.summary,
        &problem.meaning,
        problem
            .remedies
            .first()
            .map_or("", |remedy| remedy.action.as_str()),
    ))
}

/// Every finding, the ones in trouble carrying when the stack first saw them so.
///
/// Remembered first, so a fault this run is the first to see is dated by this run and
/// the next one reads the same moment back. The onset is the condition's own stamp
/// rather than the time of the run, because what an operator asks of a finding is how
/// long it has been wrong, and the health summary names the same moment for it.
#[must_use]
pub(crate) fn dated(ctx: &Ctx, found: Vec<Finding>) -> Vec<Finding> {
    let conditions = remembered(ctx, &found);
    found
        .into_iter()
        .map(|finding| {
            let onset = wrong(&finding)
                .and(conditions.get(&finding.check))
                .map(|condition| condition.since.clone());
            Finding { onset, ..finding }
        })
        .collect()
}

/// Where the store is kept: beside the environment file, or nowhere on a machine
/// with nothing configured — which has no faults to remember either.
fn path(ctx: &Ctx) -> Option<std::path::PathBuf> {
    super::targets::beside_env(ctx, "conditions.json")
}

#[cfg(test)]
mod tests;
