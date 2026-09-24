//! What this household agreed about the requests nobody rules on, kept between runs.
//!
//! The arrangement itself is [`crate::asking::Expiry`] and the reason it exists is
//! written there. This is only where it lives on disk: beside the settings, because it is
//! something the operator decided rather than something a run can work out again — and
//! because two different things read it. The clock reads it to know what it is holding the
//! household to; the household reading reads it to say so, on the list the operator reads
//! and in the message a member is handed, before anything is closed.
//!
//! **Reading is best effort and writing is not, and here the two part company sharply.** A
//! file that cannot be read is no arrangement, which closes nothing — the only safe
//! direction there is, because the other one would hold a household to a period nobody
//! could read back. A file that cannot be *written* is refused out loud: an operator told
//! their household would close requests after thirty days, by a run that recorded nothing,
//! would find every reading afterwards saying the opposite.

use crate::asking::Expiry;
use crate::error::Problem;

use super::Ctx;

/// What the arrangement is called, beside the environment file.
///
/// Named once and used from both sides: a reader and a writer disagreeing about the file
/// name would look exactly like a household that had never arranged this.
const NAME: &str = "expiring.json";

/// What this household agreed to, or nothing where it has agreed to nothing.
#[must_use]
pub(crate) fn load(ctx: &Ctx) -> Expiry {
    super::record::beside(ctx, NAME)
}

/// Write the arrangement where the next run and the next reading will find it.
///
/// # Errors
///
/// Where there is nowhere configured to keep it, or the file cannot be written.
pub(crate) fn keep(ctx: &Ctx, agreed: &Expiry) -> Result<(), Box<Problem>> {
    super::record::keep(super::targets::beside_env(ctx, NAME).as_deref(), agreed)
}

#[cfg(test)]
mod tests;
