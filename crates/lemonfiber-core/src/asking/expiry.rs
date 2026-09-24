//! How long a request may wait before nobody having ruled on it closes it.
//!
//! **There is no default and there is not going to be one.** A household nobody arranged
//! this for closes nothing: the period here is absent until an operator names one, and
//! every sentence this product says about a request that is waiting is written from that
//! absence. A figure chosen here would be this program deciding, about somebody else's
//! household, that a request had waited long enough — which is the one decision that has
//! to be handed back for an expiry to be anything other than a policy nobody agreed to.
//!
//! **It is kept because it has to be readable before it fires.** A period given to a
//! command and held nowhere would be one the household could not be told about in
//! advance, one no reading could mention, and one nobody could withdraw. So it is written
//! down beside the settings, said on the list the operator reads and in the message the
//! member is handed, and taken back by name.
//!
//! **What it cannot say is that anything is running.** lemonfiber starts nothing by
//! itself, so an arrangement recorded here is what *would* close a request rather than
//! what is closing one, and the sentences built from it say so. A record read as a
//! promise about a clock nobody started would be the background behaviour this product
//! does not have.

use serde::{Deserialize, Serialize};

/// The arrangement a household is under about the requests nobody rules on.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expiry {
    /// How many whole days a request may wait before it is closed.
    ///
    /// Absent where nothing is closed for waiting, which is what every household is
    /// under until somebody says otherwise and what one that withdrew the arrangement
    /// goes back to.
    #[serde(default)]
    after: Option<u32>,
    /// When it was agreed to, so somebody reading it later knows it is a decision
    /// somebody in this house made rather than something that arrived with an upgrade.
    ///
    /// Absent where the machine's clock could not be written as a date, which is worth
    /// keeping the period over and not worth losing it over.
    #[serde(default)]
    agreed: Option<String>,
}

impl Expiry {
    /// The arrangement an operator has just agreed to.
    #[must_use]
    pub fn agreed_to(after: u32, at: Option<String>) -> Self {
        Self {
            after: Some(after),
            agreed: at,
        }
    }

    /// How many days a request may wait, or nothing where none is closed for waiting.
    #[must_use]
    pub const fn after(&self) -> Option<u32> {
        self.after
    }

    /// The day it was agreed to, where the clock could be written down.
    #[must_use]
    pub fn agreed(&self) -> Option<&str> {
        self.agreed.as_deref()
    }

    /// Whether a period is short enough to close a request the operator was never
    /// reminded about.
    ///
    /// The reminder's own threshold is the floor, and it is a floor rather than a
    /// suggestion. A household that closed requests after three days would be reminded
    /// of nothing: the reminder is a line on a reading, and a request would be gone
    /// before any reading could carry it — so what the operator would see is requests
    /// disappearing and never one waiting on them.
    #[must_use]
    pub(crate) fn too_soon(after: u32) -> bool {
        u64::from(after) < super::REMINDING_AFTER
    }
}

#[cfg(test)]
mod tests;
