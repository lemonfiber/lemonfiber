//! Where each account stands: an invitation out or run out, a member, or switched off.
//!
//! Apart from the reading because it is the one part of it that reads the household's
//! invitations the way an offer does — the same dates, the same window, the same rule that
//! an invitation nothing can date has run out — and a second copy of that rule here would
//! be one able to call an invitation standing that the next offer takes back.

use std::collections::BTreeSet;

use crate::app::Ctx;
use crate::invitation::{offered, run_out, Offers, HOURS_OF_RECORD, HOURS_TO_CLAIM, RECORD};
use crate::model::MemberStanding;
use crate::ports::service::{Household as _, Member};

/// The invitations among these accounts that have run out, by the account's id.
///
/// Read only where there is an invitation to judge. Where the media server's record of
/// when accounts were made will not answer, nothing is called run out and the reading
/// says why: undated is what an unread record would make of every invitation, and that
/// is not what anybody found.
pub(super) async fn expired(
    ctx: &Ctx,
    server: &crate::jellyfin::Jellyfin,
    accounts: &[Member],
    findings: &mut Vec<String>,
) -> BTreeSet<String> {
    if accounts.iter().all(|account| account.claimed) {
        return BTreeSet::new();
    }
    let Ok(records) = server.when_invited(&ctx.hours_ago(HOURS_OF_RECORD)).await else {
        findings.push(
            "when the invitations were offered could not be read, so none of them is shown \
             as run out"
                .to_owned(),
        );
        return BTreeSet::new();
    };
    let offers: Offers = crate::app::record::beside(ctx, RECORD);
    let waiting = offered(accounts.to_vec(), &records, &offers);
    run_out(&waiting, &ctx.hours_ago(HOURS_TO_CLAIM))
        .every()
        .map(|gone| gone.member.id.clone())
        .collect()
}

/// Where this account stands, given which invitations have run out.
///
/// Switched off first, because it overrides the rest: an account nobody can sign in to is
/// neither a member who can nor an invitation somebody could take up.
pub(super) fn standing(account: &Member, expired: &BTreeSet<String>) -> MemberStanding {
    if account.access.disabled {
        MemberStanding::Suspended
    } else if account.claimed {
        MemberStanding::Active
    } else if expired.contains(&account.id) {
        MemberStanding::Expired
    } else {
        MemberStanding::Invited
    }
}
