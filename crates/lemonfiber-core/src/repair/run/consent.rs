//! How much of the putting-right a run was given consent for.
//!
//! A terminal holds the question open in the process that asks it: the offer is
//! printed, the operator answers, and the run that acts is the run that looked. No
//! surface reached over a network has that. The offer is read in one request and
//! the answer arrives in another, and the diagnosis the offer was built from may
//! have moved on in between.
//!
//! So consent is data here rather than a callback. It names which offer it was
//! given for, and the run that acts recomputes that name from a fresh look before
//! it carries anything out — a stale one is refused rather than spent. Nothing is
//! held between the two requests, which is also why a browser tab closed halfway
//! through leaves nothing half-consented: there is nothing to leave.

use crate::error::{Problem, Remedy, Severity};
use crate::repair::{self, Repair, Stance};

use super::{Confirm, Report};

pub use crate::error::codes::repair::STALE;

/// How much of the putting-right this run was given consent for.
///
/// Three, matching the three the command line spells: a plain run that only looks,
/// a run told in advance to carry everything out, and a run answering an offer it
/// was shown. The third is the one that has to travel, because it is the only one
/// that means anything about a particular offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Consent {
    /// Say what could be put right, and put none of it right.
    Offer,
    /// Carry out the repairs agreed to from the offer they were read in.
    Given {
        /// The offer they were read in, as it named itself.
        offer: String,
        /// The checks whose repairs were agreed to, as the offer names them.
        repairs: Vec<String>,
    },
    /// Carry them out without asking, because this run was told to in advance.
    Standing,
}

impl Consent {
    /// How this run may act, which is what the sequence asks about.
    #[must_use]
    pub const fn stance(&self) -> Stance {
        match self {
            Self::Offer => Stance::ReportOnly,
            Self::Given { .. } => Stance::Ask,
            Self::Standing => Stance::Unattended,
        }
    }

    /// Nothing, or why what was agreed to cannot be spent on what is offered now.
    ///
    /// Read from the report rather than from the offer directly, so the comparison
    /// is against the very list the run would have acted on.
    ///
    /// # Errors
    ///
    /// Returns the [`Problem`] a surface should answer with where the offer has
    /// moved on since it was read.
    pub fn held(&self, report: &Report) -> Result<(), Box<Problem>> {
        match self {
            Self::Given { offer, .. } if *offer != report.agreement => {
                Err(Box::new(stale(offer, &report.agreement)))
            }
            Self::Offer | Self::Given { .. } | Self::Standing => Ok(()),
        }
    }
}

impl Confirm for Consent {
    /// Whether this repair is one of the ones agreed to.
    ///
    /// By the check its finding names, which is what an offer gives an operator to
    /// point at. Only a consent given for an offer answers yes to anything: the
    /// other two are never asked, because their stance does not ask.
    fn agreed(&self, repair: &Repair) -> bool {
        match self {
            Self::Given { repairs, .. } => repairs.contains(&repair.check),
            Self::Offer | Self::Standing => false,
        }
    }

    /// Whether the offer this was given for is the offer that stands now.
    fn stands(&self, offered: &[Repair]) -> bool {
        match self {
            Self::Given { offer, .. } => *offer == repair::agreement(offered),
            Self::Offer | Self::Standing => true,
        }
    }
}

/// The refusal for consent given to an offer that has since moved on.
///
/// Both names are said. An operator who reads only that something changed cannot
/// tell a repair whose effects were rewritten from a fault that has cleared, and
/// the two ask for opposite things next.
fn stale(agreed: &str, stands: &str) -> Problem {
    Problem::new(
        STALE,
        Severity::Warning,
        "What you agreed to is not what is offered now",
        format!(
            "The offer you answered was {agreed}, and a fresh look offers {stands}. \
             Something has changed since you read it, so nothing was carried out."
        ),
        Remedy::new("Ask what could be put right again, and read what it says now"),
    )
}

#[cfg(test)]
mod tests;
