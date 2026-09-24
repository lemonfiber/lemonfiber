//! What the operator has been told, and what is still owed them, kept between
//! runs.
//!
//! The outbox was written to survive a channel that was down — an alert decided
//! and not yet delivered is held rather than dropped — and it could not, because
//! nothing wrote it anywhere. Every run started with an empty one, which means a
//! fault that arrived while a channel was refusing was owed until the process
//! ended and then forgotten, and a condition that resolved before anybody read it
//! left no history at all.
//!
//! Kept beside the configuration, with the conditions it is read against: the two
//! are one picture of what has happened, and a restore that brought back one
//! without the other would report every standing fault as new.

use crate::alert::Outbox;

use super::Ctx;

/// What the last run left, or an empty outbox where there is none.
///
/// An unreadable one costs the operator a repeat of something they were already
/// told, which is tiresome and the safe direction — the alternative is a fault
/// nobody hears about because a file said it had been delivered.
#[must_use]
pub(crate) fn load(ctx: &Ctx) -> Outbox {
    super::record::kept(path(ctx).as_deref())
}

/// Write it where the next run will read it.
pub(crate) fn save(ctx: &Ctx, outbox: &Outbox) {
    // Best effort, like the conditions it accompanies: a refresh that could not
    // write its history is one the next refresh starts afresh from.
    let _ = super::record::keep(path(ctx).as_deref(), outbox);
}

/// Where it is kept: beside the environment file, or nowhere on a machine with
/// nothing configured — which has nobody to owe anything to either.
fn path(ctx: &Ctx) -> Option<std::path::PathBuf> {
    super::targets::beside_env(ctx, "outbox.json")
}

#[cfg(test)]
mod tests;
