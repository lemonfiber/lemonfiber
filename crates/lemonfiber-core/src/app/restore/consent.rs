//! How much of a restore a run was given consent for.
//!
//! A restore is two answers with the operator's decision in the gap: what the
//! archive holds, read before anything is overwritten, and then the overwrite. An
//! operator at a shell is present for both, and the run that overwrites is the run
//! that listed. A surface reached over a network is not: the listing is read in one
//! request and the yes arrives in another, and what the archive would do may have
//! moved on in between — the data root it would be re-pointed to is derived afresh
//! each time, from settings anything on this machine may have changed since.
//!
//! So consent names the listing it was given for, and the run that overwrites
//! builds that name again from a fresh look before it touches anything. A yes given
//! for one listing is refused rather than spent on another — and the refusal
//! matters most where the difference is invisible afterwards, because what comes
//! back with a finished restore is the listing that was acted on, not the one that
//! was read.
//!
//! This is a race and replay guard, not a permission. Whoever can send the second
//! request could have sent the first.

use crate::error::{Problem, Remedy, Severity, State};

use super::Preview;

pub use crate::error::codes::restore::MOVED_ON;

/// How much of a restore this run was given consent for.
///
/// Three, matching the three a repair's consent has and for the same reasons. A
/// run that only lists, a run answering the listing it was shown, and a run told to
/// overwrite before there was a listing to read.
///
/// The third is not a hole in the second. It is what a surface holding the question
/// open in one process has — a shell where the operator typed the agreement in
/// advance, and a screen that lists and then asks without leaving the process — and
/// there is nothing for such a run to name, because the listing it acts on is the
/// one it just made. A surface whose yes crossed a request boundary has the second,
/// which is the only one that says anything about a particular listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Consent {
    /// Say what the archive holds, and overwrite nothing.
    List,
    /// Overwrite, having read the listing this names.
    Given {
        /// The listing it was read in, as that listing named itself.
        listing: String,
    },
    /// Overwrite, because this run was told to before there was a listing to read.
    Standing,
}

impl Consent {
    /// Whether this run overwrites anything, which is the fork inside the command.
    #[must_use]
    pub const fn overwrites(&self) -> bool {
        !matches!(self, Self::List)
    }

    /// Nothing, or why what was agreed to cannot be spent on what the archive would
    /// do now.
    ///
    /// Read from the listing this run built rather than from the archive directly,
    /// so the comparison is against the very account the restore would act on.
    ///
    /// # Errors
    ///
    /// Returns the [`Problem`] a surface should answer with where the listing has
    /// moved on since it was read.
    pub fn held(&self, would: &Preview) -> Result<(), Box<Problem>> {
        match self {
            Self::Given { listing } if *listing != would.agreement => {
                Err(Box::new(moved_on(listing, &would.agreement)))
            }
            Self::List | Self::Given { .. } | Self::Standing => Ok(()),
        }
    }
}

/// The refusal for consent given to a listing that has since moved on.
///
/// Both names are said, for the reason a stale repair offer says both: an operator
/// who reads only that something changed cannot tell an archive that was replaced
/// from a data root that moved under them, and the two ask for opposite things
/// next.
fn moved_on(agreed: &str, stands: &str) -> Problem {
    Problem::new(
        MOVED_ON,
        Severity::Warning,
        "What you agreed to is not what this backup would do now",
        format!(
            "The listing you answered was {agreed}, and a fresh look at the archive lists \
             {stands}. Something has changed since you read it, so nothing was overwritten."
        ),
        Remedy::new("Ask what the backup holds again, and read what it says now"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests;
