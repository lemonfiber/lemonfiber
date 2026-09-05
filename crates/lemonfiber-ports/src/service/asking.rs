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

/// What taking one member's asking away came to, kept only so it can be given back.
///
/// **Opaque on purpose.** A request service tells several kinds of "may ask" apart — a
/// plain one, one for each kind of thing, one for the higher quality — and this product
/// has no vocabulary for that set. Naming them here would be a second reading of somebody
/// else's permission model, able to disagree with the one that actually refuses a
/// request; carrying the service's own answer back out unread cannot. So a member who
/// could only ask for one kind of thing is given that back rather than a plain grant this
/// side decided was equivalent.
///
/// Written down between runs by whoever holds it, because what is taken while a disk is
/// full has to survive until there is room again.
#[derive(Debug, Default)]
pub struct Holding {
    /// What was taken, in the service's own numbering.
    pub taken: u64,
}

impl Holding {
    /// Whether anything was taken at all.
    ///
    /// Nothing taken is nobody to give anything back to, which is a different thing from
    /// a member held back with no bits to their name.
    #[must_use]
    pub const fn anything(&self) -> bool {
        self.taken != 0
    }
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

    /// Take away what lets this member ask for anything at all, and answer with what
    /// was taken.
    ///
    /// **Not a quota, and deliberately not reachable through one.** A limit of nought is
    /// not a state a request service can be put into, and one that could would refuse in
    /// the words of a limit — which is the one sentence a disk with no room must not be
    /// mistaken for. This stops the asking itself, so what refuses says permission and
    /// never says quota.
    ///
    /// **Nothing else about the account is touched**, and nothing is taken from an
    /// account the service treats as holding every permission: the gate reads that first
    /// and answers yes whatever else is set, so a bit taken off the owner would block
    /// nothing and a bit given back would be a change made for no effect.
    ///
    /// Answers with nothing taken where there was nothing to take — a member already
    /// held back, an owner, or somebody this service has never heard of.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn hold_requests(&self, id: &str) -> Result<Holding, Failure>;

    /// Give back exactly what [`Approving::hold_requests`] took, and nothing else.
    ///
    /// Exactly what was taken rather than a grant composed here, so a permission an
    /// operator narrowed by hand comes back the shape they left it. Anything they took
    /// away *since* stays taken away: this puts back, it does not restore.
    ///
    /// A member this service no longer holds an account for is nothing to give back to
    /// rather than a failure to give something back — otherwise a household would carry
    /// a record of somebody who left for as long as it existed.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn release_requests(&self, id: &str, holding: Holding) -> Result<(), Failure>;
}

#[cfg(test)]
mod tests {
    use super::{Holding, Left};

    /// Taking nothing is told from taking something, which is what decides whether
    /// anybody is written down as held back at all.
    #[test]
    fn taking_nothing_is_not_a_member_held_back() {
        let nothing = Holding::default();
        let something = Holding { taken: 32 };

        assert!(!nothing.anything());
        assert!(something.anything());
        // Formatted rather than left to an assertion's message, which is evaluated only
        // where the assertion fails: what carries the service's own number has to be
        // readable in a report, and a rendering nothing runs is not.
        assert_eq!(format!("{something:?}"), "Holding { taken: 32 }");
    }

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
