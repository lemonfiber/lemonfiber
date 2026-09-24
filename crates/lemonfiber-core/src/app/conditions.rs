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

use crate::condition::Conditions;

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

/// Write the store where the next run will read it.
pub fn save(ctx: &Ctx, conditions: &Conditions) {
    // Best effort on the way out, unlike the records an operator decided: a
    // refresh that could not write its history is one the next refresh starts
    // afresh from, which is a worse picture rather than a wrong claim.
    let _ = super::record::keep(path(ctx).as_deref(), conditions);
}

/// Where the store is kept: beside the environment file, or nowhere on a machine
/// with nothing configured — which has no faults to remember either.
fn path(ctx: &Ctx) -> Option<std::path::PathBuf> {
    super::targets::beside_env(ctx, "conditions.json")
}

#[cfg(test)]
mod tests;
