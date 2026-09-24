//! Why requests were turned down, kept between runs.
//!
//! The record itself is [`crate::asking::Reasons`] and the reason it exists is written
//! there. This is only where it lives on disk: beside the settings, because it is a
//! record of something the operator decided rather than something a run can work out
//! again, and a restore that lost it would leave every refusal bare — which is the state
//! the record exists to get a household out of.
//!
//! **Best effort both ways, unlike the choices an operator answered.** A record that
//! cannot be read costs the words beside one refusal, which is a worse message rather
//! than a wrong one; and a decision already carried out at the request service must not
//! fail on the way back because a note could not be written. What that failure costs is
//! said where it happens instead.

use crate::asking::Reasons;
use crate::error::Problem;

use super::Ctx;

/// What the record is called, beside the environment file.
///
/// Named once and used from both sides: a reader and a writer disagreeing about the file
/// name would look exactly like a household nobody had ever refused anything.
const NAME: &str = "refusals.json";

/// Every reason this machine holds, or none where nothing has been turned down here.
#[must_use]
pub(crate) fn load(ctx: &Ctx) -> Reasons {
    super::record::beside(ctx, NAME)
}

/// Write the reasons where the next run will find them.
///
/// # Errors
///
/// Where there is nowhere configured to keep them, or the file cannot be written. Said
/// rather than swallowed, unlike the histories a run can work out again: what is lost
/// here is the only copy of somebody's words, and an operator told a reason is theirs to
/// pass on while nothing kept it would find it gone the next time they looked.
pub(crate) fn keep(ctx: &Ctx, reasons: &Reasons) -> Result<(), Box<Problem>> {
    super::record::keep(super::targets::beside_env(ctx, NAME).as_deref(), reasons)
}

#[cfg(test)]
mod tests;
