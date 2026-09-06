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
    pub fn too_soon(after: u32) -> bool {
        u64::from(after) < super::REMINDING_AFTER
    }
}

#[cfg(test)]
mod tests {
    use super::Expiry;

    /// A moment the calendar holds, for these records to be stamped with.
    const AT: &str = "2026-08-17T21:04:09";

    /// A household nobody arranged this for closes nothing.
    #[test]
    fn nothing_is_arranged_until_somebody_arranges_it() {
        let fresh = Expiry::default();

        assert_eq!(fresh.after(), None);
        assert_eq!(fresh.agreed(), None);
    }

    /// What was agreed to comes back, with the day it was agreed on.
    #[test]
    fn what_was_agreed_to_comes_back_with_the_day_it_was_agreed_on() {
        let agreed = Expiry::agreed_to(30, Some(AT.to_owned()));

        assert_eq!(agreed.after(), Some(30));
        assert_eq!(agreed.agreed(), Some(AT));
    }

    /// A clock that could not be written down costs the day and not the period.
    #[test]
    fn a_clock_that_could_not_be_written_costs_the_day_and_not_the_period() {
        let agreed = Expiry::agreed_to(30, None);

        assert_eq!(agreed.after(), Some(30));
        assert_eq!(agreed.agreed(), None);
    }

    /// It survives being written down and read back.
    #[test]
    fn it_survives_being_written_down_and_read_back() {
        let agreed = Expiry::agreed_to(45, Some(AT.to_owned()));

        let written = serde_json::to_string(&agreed).unwrap_or_default();
        let read: Expiry = serde_json::from_str(&written).unwrap_or_default();

        assert_eq!(read, agreed, "{written}");
    }

    /// A record this cannot read is no arrangement rather than a failure.
    ///
    /// The safe direction, and the only safe direction there is: a file that will not
    /// parse leaving a period in force would hold a household to a number nobody could
    /// read back.
    #[test]
    fn a_record_that_will_not_read_is_no_arrangement() {
        let read: Expiry = serde_json::from_str("not a record").unwrap_or_default();

        assert_eq!(read.after(), None);
    }

    /// A period shorter than the reminder is one nothing is ever reminded about.
    #[test]
    fn a_period_shorter_than_the_reminder_is_too_soon() {
        assert!(Expiry::too_soon(0));
        assert!(Expiry::too_soon(6));
        assert!(
            !Expiry::too_soon(7),
            "the reminder's own threshold is allowed"
        );
        assert!(!Expiry::too_soon(30));
    }
}
