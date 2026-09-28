//! What an invitation is, and when it has run out.
//!
//! An invitation is an account with no password on it: made by the operator, claimed by
//! whoever sets the first password, and taken back if nobody does. The account itself is
//! the media server's; **when it was offered** is written down here as well, because the
//! server's own record of it cannot be relied on to still hold it.
//!
//! **An invitation nobody can date has run out.** The server's record of when an account
//! was made is bounded — it keeps a month, and a read of it is cut at a few hundred entries
//! — and anybody on the network can push an entry out of it by getting a password wrong
//! often enough. An account that fell out of it would otherwise stand claimable for as long
//! as nobody noticed. So the date comes first from what this program recorded when it made
//! the offer, then from what the server still remembers, and an unclaimed account neither
//! can date is treated as expired: the direction to err in, since what is at stake either
//! way is an account nobody has claimed.
//!
//! **An invitation is dated by whenever it was last offered**, which is not always when
//! the account was made. A reset returns an existing account to having no password, and
//! that is the account being offered again — months after it was created. So the latest
//! of every date on record is the one taken.
//!
//! **What running out costs depends on whose account it is.** An offer nobody ever took
//! up is an account with nothing on it, and it is removed. An account somebody has been
//! in — reset, and not claimed again in time — holds what they watched, and removing it
//! would take that with it; so it is switched off instead, and kept for a reissue to
//! switch back on.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ports::service::{Invited, Member};

/// How long an invitation stands before it is withdrawn.
pub(crate) const HOURS_TO_CLAIM: i64 = 48;

/// How far back the server's record is read when dating invitations.
///
/// **This is not the same moment as [`HOURS_TO_CLAIM`] and must be longer.** The
/// server answers with what happened *since* the moment it is given, and the
/// invitations being looked for are the ones already past their window — so
/// reading from the same moment they are judged against returns only the ones
/// still standing, and every older one would read as undated.
pub(crate) const HOURS_OF_RECORD: i64 = 24 * 30;

/// Where the record of what was offered is kept, beside the configuration.
pub(crate) const RECORD: &str = "invitations.json";

/// What this program offered, by the account it offered it on.
pub(crate) type Offers = BTreeMap<String, Offer>;

/// One offer, as this program recorded making it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Offer {
    /// When it was offered, written the way the server writes a moment.
    pub(crate) offered: String,
    /// When it runs out.
    pub(crate) lapses: String,
}

/// An account nobody has claimed, with when it was offered where that is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    /// The account it is about.
    pub member: Member,
    /// When it was offered, absent where nothing records it.
    pub at: Option<String>,
}

/// The invitations among a household: the accounts somebody could still claim.
///
/// A claimed account is a member, not an invitation. Neither is one that is switched off,
/// which nobody can sign in to and so nobody can claim; nor the administrator, which is
/// the account this program signs in as and is never offered to anybody.
#[must_use]
pub(crate) fn offered(
    household: Vec<Member>,
    invited: &[Invited],
    offers: &Offers,
) -> Vec<Offered> {
    household
        .into_iter()
        .filter(|member| !member.claimed && !member.access.disabled && !member.access.administrator)
        .map(|member| {
            // The **latest** of them, not the first: an account carries a record of being
            // made and one for every time a password moved off it, and what dates the
            // invitation is whichever happened last.
            let at = invited
                .iter()
                .filter(|record| record.member == member.id)
                .map(|record| record.at.clone())
                .chain(offers.get(&member.id).map(|offer| offer.offered.clone()))
                .filter(|at| comparable(at))
                .max();
            Offered { member, at }
        })
        .collect()
}

/// The invitations that have run out, and what becomes of each.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Spent {
    /// Offers nobody ever took up, which are removed.
    pub(crate) withdrawn: Vec<Offered>,
    /// Accounts somebody has been in, which are switched off and kept.
    pub(crate) suspended: Vec<Offered>,
}

impl Spent {
    /// Every one of them, whichever becomes of it.
    pub(crate) fn every(&self) -> impl Iterator<Item = &Offered> {
        self.withdrawn.iter().chain(&self.suspended)
    }
}

/// The invitations that have run out: offered before `cutoff`, or never dated at all.
///
/// `cutoff` and each date are compared as text, which sorts correctly because each is an
/// ISO-8601 instant ending in `Z`. A date that is not one was dropped by [`offered`], so an
/// invitation carrying none here is one nothing could date.
#[must_use]
pub(crate) fn run_out(offered: &[Offered], cutoff: &str) -> Spent {
    let mut spent = Spent::default();
    for invitation in offered {
        if invitation.at.as_deref().is_some_and(|at| at >= cutoff) {
            continue;
        }
        if invitation.member.last_seen.is_some() {
            spent.suspended.push(invitation.clone());
        } else {
            spent.withdrawn.push(invitation.clone());
        }
    }
    spent
}

/// The record with one offer written into it, and the ones no longer standing dropped.
///
/// What is dropped is every account that is not an invitation any more — claimed, or gone —
/// so the record holds the offers still out and nothing else.
#[must_use]
pub(crate) fn recorded(
    mut offers: Offers,
    household: &[Member],
    member: &str,
    offer: Offer,
) -> Offers {
    offers.retain(|id, _| household.iter().any(|held| &held.id == id && !held.claimed));
    offers.insert(member.to_owned(), offer);
    offers
}

/// Whether a recorded moment is one this can order against another.
///
/// The server writes an ISO-8601 instant ending in `Z`, and so does this program. Anything
/// else — a local time, an empty string, a shape a later version invents — is not
/// something to compare as text.
fn comparable(moment: &str) -> bool {
    moment.ends_with('Z') && moment.len() >= "2026-08-29T00:00:00Z".len()
}

#[cfg(test)]
mod tests;
