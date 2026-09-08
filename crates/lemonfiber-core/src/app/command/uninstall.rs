//! What one run was asked to take off this machine.
//!
//! One value rather than four fields on the dispatcher's table, for the reason the
//! hosting request beside it is one: what an operator is asking when they ask to
//! remove something is a single decision with parts, and spelling the parts out at
//! the routing table would put the longest arm in it on the request that must be
//! read most carefully.

use crate::app::Waiting;
use crate::uninstall::Tier;

/// What one run was asked to remove, and what it was answered with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removing {
    /// Which of the four removals.
    pub tier: Tier,
    /// Whether the operator said to go ahead.
    ///
    /// On its own this is enough for the three tiers that leave the library alone.
    /// It is never enough for the one that does not — see [`Self::agreement`].
    pub confirm: bool,
    /// The name of the reading the operator agreed to, where they gave one.
    ///
    /// The tier that takes the library takes nothing without one: a bare yes could
    /// have been given against a disk that has since changed, and what it would
    /// destroy cannot be fetched again.
    pub agreement: Option<String>,
    /// Whether anything still coming down is let finish before the services stop.
    pub waiting: Waiting,
}

impl Removing {
    /// A reading, changing nothing — what a run that named a tier and no more asks
    /// for.
    #[must_use]
    pub fn surveying(tier: Tier) -> Self {
        Self {
            tier,
            confirm: false,
            agreement: None,
            waiting: Waiting::Never,
        }
    }

    /// The same request, gone ahead with.
    #[must_use]
    pub fn confirmed(mut self, confirm: bool) -> Self {
        self.confirm = confirm;
        self
    }

    /// The same request, answering a reading by the name it printed.
    #[must_use]
    pub fn agreeing(mut self, agreement: Option<String>) -> Self {
        self.agreement = agreement;
        self
    }

    /// The same request, letting what is coming down finish first.
    #[must_use]
    pub fn waiting(mut self, waiting: Waiting) -> Self {
        self.waiting = waiting;
        self
    }

    /// Whether this run may remove anything at all.
    ///
    /// A rehearsal is not an answer, which every other write in this product also
    /// holds; that half is the caller's, because a rehearsal still reports what a
    /// confirmed run would take.
    #[must_use]
    pub const fn goes_ahead(&self) -> bool {
        self.confirm
    }
}

#[cfg(test)]
mod tests {
    use super::Removing;
    use crate::app::Waiting;
    use crate::uninstall::Tier;

    #[test]
    fn a_bare_request_reads_and_agrees_to_nothing() {
        let asked = Removing::surveying(Tier::Services);

        assert!(!asked.goes_ahead());
        assert_eq!(asked.agreement, None);
        assert_eq!(asked.waiting, Waiting::Never);
        assert_eq!(asked.tier, Tier::Services);
    }

    #[test]
    fn each_part_of_the_request_is_carried_as_it_was_given() {
        let asked = Removing::surveying(Tier::Media)
            .confirmed(true)
            .agreeing(Some("deadbeef".to_owned()))
            .waiting(Waiting::ForTheDownloads);

        assert!(asked.goes_ahead());
        assert_eq!(asked.agreement.as_deref(), Some("deadbeef"));
        assert_eq!(asked.waiting, Waiting::ForTheDownloads);
        assert_eq!(asked.clone(), asked);
        assert!(format!("{asked:?}").contains("Media"));
    }

    #[test]
    fn a_request_that_was_not_confirmed_is_not_one_that_goes_ahead() {
        let asked = Removing::surveying(Tier::Configuration).confirmed(false);

        assert!(!asked.goes_ahead());
        assert_ne!(asked, Removing::surveying(Tier::Media));
    }
}
