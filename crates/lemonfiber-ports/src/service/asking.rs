//! What a household may ask for, and what deciding one request comes to.
//!
//! Apart from [`Requests`](super::Requests) because it is a different errand. That port is
//! how the request service is set up and read — pointed at a media server, handed the
//! \*arrs, asked what the household has asked for. This is what the household is *allowed*
//! to ask for and what an operator does about one request, which is a decision rather
//! than a wiring.
//!
//! **The counts are the service's own, and it keeps two of them.** Film and television
//! are counted separately, and television is counted in seasons rather than in requests —
//! so one ask for a six-season series spends six. Carried as the two the service keeps
//! rather than folded into one here, because folding them would report a household as
//! within its limit while the half that matters is spent.
//!
//! Nothing here is written down on this machine. What a member has left is the service's
//! own arithmetic over its own records, read back rather than counted again, so a second
//! count cannot disagree with the one that actually refuses a request.

use async_trait::async_trait;

use super::Failure;

/// How much may be asked for in a period.
///
/// Two numbers and no more: how many, and over how long. Expressed in the terms a
/// household says them in rather than in an internal counter — "five a week" is the
/// sentence, and both halves of it are here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quota {
    /// How many requests the period allows.
    pub requests: u32,
    /// How long the period is, in days.
    pub days: u32,
}

/// What one kind of request has cost a member, and what the period still allows.
///
/// The limit is absent where nothing holds them to one, which is a different answer
/// from a limit of nought: nought would be a member who may ask for nothing, and the
/// service spells "no limit" that way. Read back rather than derived, so what is
/// reported is the arithmetic that will actually refuse the next request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Left {
    /// How many the period allows. `None` where nothing limits them.
    pub limit: Option<u32>,
    /// How many the period has already counted.
    pub used: u32,
    /// How long the period is, in days. `None` where the count runs from the
    /// beginning rather than over a window.
    pub days: Option<u32>,
}

impl Left {
    /// How many more may be asked for, or `None` where nothing limits them.
    ///
    /// Saturating, because a limit lowered under what has already been spent leaves a
    /// member over it — which is nought left rather than a negative number of requests.
    #[must_use]
    pub const fn remaining(self) -> Option<u32> {
        match self.limit {
            None => None,
            Some(limit) => Some(limit.saturating_sub(self.used)),
        }
    }

    /// Whether the next request of this kind would be refused.
    #[must_use]
    pub const fn spent(self) -> bool {
        matches!(self.remaining(), Some(0))
    }
}

/// What one member has left, in each of the two kinds the service counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Headroom {
    /// Films, counted one to a request.
    pub films: Left,
    /// Television, counted one to a season — so one ask for a six-season series
    /// spends six.
    pub television: Left,
}

/// What the household may ask for where nobody has chosen otherwise for one person.
///
/// The service holds these together with everything else about itself, so both are read
/// and written as one: a household that auto-approves within a limit is one setting made
/// of two halves, and writing either alone leaves the other saying something the operator
/// did not choose.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Asking {
    /// Whether what a member asks for arrives without anybody seeing it first.
    pub approves_own: bool,
    /// How much may be asked for in a period, or `None` where nothing limits it.
    pub quota: Option<Quota>,
}

/// Deciding what the household may ask for, and deciding one request that is waiting.
///
/// Apart from [`Requests`](super::Requests) because setting a service up and ruling on
/// what a person asked for are different errands, reached by different commands and at
/// different moments.
#[async_trait]
pub trait Approving: Send + Sync {
    /// What the household may ask for where nobody chose otherwise for one person.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn asking(&self) -> Result<Asking, Failure>;

    /// Set what the household may ask for where nobody chose otherwise.
    ///
    /// **Only the two halves named are written.** The service merges what it is sent
    /// into what it holds, so everything else about it — where the media server is, what
    /// it tells the household about — is left exactly as it was.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn set_asking(&self, asking: &Asking) -> Result<(), Failure>;

    /// What one member has left of their period, by the service's own identifier.
    ///
    /// The service's own arithmetic over its own records rather than a count made here:
    /// what is reported has to be what will actually refuse the next request, and a
    /// second count is a second answer able to disagree with the one that matters.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn left(&self, id: &str) -> Result<Headroom, Failure>;

    /// Hold one member to a quota of their own, or take theirs away so the
    /// household's applies again.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn set_quota(&self, id: &str, quota: Option<Quota>) -> Result<(), Failure>;

    /// Set whether what this member asks for arrives without anybody seeing it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn approves_own(&self, id: &str, may: bool) -> Result<(), Failure>;

    /// Let one waiting request through, or turn it down.
    ///
    /// **The service takes no reason.** Its endpoint carries the decision in the path
    /// and reads no body at all, and the record it keeps has no field for one — so a
    /// reason given here is the operator's to pass on rather than something this can
    /// deliver. What the service does send the requester is that it was declined.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable, holds no such request, or
    /// refuses.
    async fn decide(&self, request: i64, approve: bool) -> Result<(), Failure>;
}

#[cfg(test)]
mod tests {
    use super::Left;

    /// What is left is the limit less what has been spent.
    #[test]
    fn what_is_left_is_the_limit_less_what_was_spent() {
        let held = Left {
            limit: Some(5),
            used: 2,
            days: Some(7),
        };

        assert_eq!(held.remaining(), Some(3));
        assert!(!held.spent());
    }

    /// A member nothing limits has no number left, which is not nought left.
    ///
    /// Nought would be somebody who may ask for nothing. Reported as the same figure,
    /// an unlimited member would read as the most restricted one in the house.
    #[test]
    fn a_member_nothing_limits_has_no_figure_rather_than_nought() {
        let open = Left {
            limit: None,
            used: 40,
            days: None,
        };

        assert_eq!(open.remaining(), None);
        assert!(!open.spent());
    }

    /// A limit lowered under what is already spent leaves nought, never a wrap.
    #[test]
    fn a_limit_lowered_under_what_is_spent_leaves_nought() {
        let over = Left {
            limit: Some(2),
            used: 9,
            days: Some(30),
        };

        assert_eq!(over.remaining(), Some(0));
        assert!(over.spent());
    }
}
