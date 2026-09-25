//! What an invitation is, and when it has run out.
//!
//! An invitation is an account with no password on it. That is the whole of it:
//! made by the operator, claimed by whoever sets the first password, and withdrawn
//! if nobody does. Nothing about it is written down here — the media server already
//! holds both halves, so an invitation outlives this program being closed,
//! reinstalled, or run from another machine.
//!
//! **An invitation nobody can date is left alone.** The record of when an account
//! was made is kept apart from the account itself and does not last forever, so an
//! unclaimed account with no such record is one this cannot reason about — somebody
//! made it by hand, or it is older than what the server still remembers. Withdrawing
//! it on a guess would take away an account somebody is about to use, which is worse
//! than leaving one standing a while longer.
//!
//! **An invitation is dated by whenever it was last offered**, which is not always when
//! the account was made. A password reset returns an existing account to having none,
//! and that is the account being offered again — months after it was created. So the
//! record read here is the latest of them, and a reset invitation runs out from the
//! reset rather than being expired the instant it is made.

use crate::ports::service::{Invited, Member};

/// How long an invitation stands before it is withdrawn.
pub(crate) const HOURS_TO_CLAIM: i64 = 48;

/// How far back the record is read when dating invitations.
///
/// **This is not the same moment as [`HOURS_TO_CLAIM`] and must be longer.** The
/// server answers with what happened *since* the moment it is given, and the
/// invitations being looked for are the ones already past their window — so
/// reading from the same moment they are judged against returns only the ones
/// still standing, and nothing is ever found to withdraw.
///
/// Beyond this the record is not read, and an invitation it does not cover is one
/// nothing can date, which is left standing rather than withdrawn on a guess. The
/// server trims its own record eventually, so no window makes that case go away.
pub(crate) const HOURS_OF_RECORD: i64 = 24 * 30;

/// An account nobody has claimed, with when it was offered where that is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    /// The account it is about.
    pub member: Member,
    /// When it was made, absent where nothing records it any longer.
    pub at: Option<String>,
}

/// The invitations among a household: the accounts nobody has claimed.
///
/// A claimed account is a member, not an invitation, and is not reported here — the
/// operator asking who has not joined yet is asking a different question from who
/// is in the house.
#[must_use]
pub fn offered(household: Vec<Member>, invited: &[Invited]) -> Vec<Offered> {
    household
        .into_iter()
        .filter(|member| !member.claimed)
        .map(|member| {
            // The **latest** of them, not the first: an account carries a record of
            // being made and one for every time a password moved on or off it, and
            // what dates the invitation is whichever happened last. An account whose
            // password was taken off was offered again at that moment, months after it
            // was made — dated by its creation it would already be long expired.
            let at = invited
                .iter()
                .filter(|record| record.member == member.id)
                .map(|record| record.at.clone())
                .max();
            Offered { member, at }
        })
        .collect()
}

/// The invitations that have run out, and may be withdrawn.
///
/// `now` and each `at` are compared as the server writes them, which sorts
/// correctly as text because they are fixed-width and end in `Z`. Comparing the
/// strings rather than parsing them keeps this free of a clock and of a date
/// library, and a record this cannot compare is one it declines to act on.
///
/// **An account whose password was taken off runs out from that moment**, not from when
/// the account was made. Unclaimed no longer means new: a reset puts an existing account
/// back to having no password, and dating that by its creation would find it expired the
/// instant it was reset — so the very next invitation offered to anybody would withdraw
/// it. What makes the window real is [`offered`] taking the latest record rather than the
/// first.
///
/// What expiry costs here is worth naming: withdrawing removes the account, and for
/// somebody who was in the household that takes their watch history with it. An
/// invitation nobody claims does not stand, whichever kind it is, and that is why the
/// message an operator passes on says what running out costs rather than only that it
/// happens.
#[must_use]
pub(crate) fn run_out<'a>(offered: &'a [Offered], cutoff: &str) -> Vec<&'a Offered> {
    offered
        .iter()
        .filter(|invitation| {
            invitation
                .at
                .as_deref()
                .is_some_and(|made| comparable(made) && made < cutoff)
        })
        .collect()
}

/// Whether a recorded moment is one this can order against another.
///
/// The server writes an ISO-8601 instant ending in `Z`. Anything else — a local
/// time, an empty string, a shape a later version invents — is not something to
/// compare as text, and an invitation carrying one is left standing.
fn comparable(moment: &str) -> bool {
    moment.ends_with('Z') && moment.len() >= "2026-08-29T00:00:00Z".len()
}

#[cfg(test)]
mod tests;
