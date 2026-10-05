//! Where each account stands: an invitation out or run out, a member, or switched off.
//!
//! Apart from the reading because it is the one part of it that reads the household's
//! invitations the way an offer does — the same dates, the same window, the same rule that
//! an invitation nothing can date has run out — and a second copy of that rule here would
//! be one able to call an invitation standing that the next offer takes back.

use std::collections::BTreeSet;

use crate::app::Ctx;
use crate::invitation::{
    closed, offered, run_out, Offers, Spent, HOURS_OF_RECORD, HOURS_TO_CLAIM, RECORD,
};
use crate::model::MemberStanding;
use crate::ports::service::{Household as _, Member};

/// Where the invitations among these accounts stand, with what has run out taken back.
///
/// Which have run out and which were declined, by the account's id: the run-out ones
/// read before any was taken back, so this reading still says what it found.
pub(super) async fn invitations(
    ctx: &Ctx,
    server: &crate::jellyfin::Jellyfin,
    accounts: &[Member],
    findings: &mut Vec<String>,
) -> (BTreeSet<String>, BTreeSet<String>) {
    let spent = expired(ctx, server, accounts, findings).await;
    let declined = declined(ctx, server, accounts, findings).await;
    close_taken_up(ctx, accounts);
    taken_back(ctx, server, &spent, &declined, findings).await;
    let expired = spent.every().map(|gone| gone.member.id.clone()).collect();
    (expired, declined)
}

/// The invitations among these accounts that have run out.
///
/// Read only where there is an invitation to judge. Where the media server's record of
/// when accounts were made will not answer, nothing is called run out and the reading
/// says why: undated is what an unread record would make of every invitation, and that
/// is not what anybody found.
async fn expired(
    ctx: &Ctx,
    server: &crate::jellyfin::Jellyfin,
    accounts: &[Member],
    findings: &mut Vec<String>,
) -> Spent {
    if accounts.iter().all(|account| account.claimed) {
        return Spent::default();
    }
    let Ok(records) = server.when_invited(&ctx.hours_ago(HOURS_OF_RECORD)).await else {
        findings.push(
            "when the invitations were offered could not be read, so none of them is shown \
             as run out"
                .to_owned(),
        );
        return Spent::default();
    };
    let offers: Offers = crate::app::record::beside(ctx, RECORD);
    let waiting = offered(accounts.to_vec(), &records, &offers);
    run_out(&waiting, &ctx.hours_ago(HOURS_TO_CLAIM))
}

/// Take back the invitations that have run out, the way an offer sweeps them, and say
/// which could not be.
///
/// Tidying rather than protection: an invitation that ran out is refused at the door
/// whether or not this ever runs. What it adds is the media server refusing it too, and
/// it does that the moment anybody reads the household rather than waiting for the next
/// invitation. A declined one is left for the operator, as an offer leaves it, and a
/// rehearsal takes back nothing.
async fn taken_back(
    ctx: &Ctx,
    server: &crate::jellyfin::Jellyfin,
    spent: &Spent,
    declined: &BTreeSet<String>,
    findings: &mut Vec<String>,
) {
    if ctx.dry_run {
        return;
    }
    let keep = |gone: &crate::invitation::Offered| !declined.contains(&gone.member.id);
    let due = Spent {
        withdrawn: spent
            .withdrawn
            .iter()
            .filter(|gone| keep(gone))
            .cloned()
            .collect(),
        suspended: spent
            .suspended
            .iter()
            .filter(|gone| keep(gone))
            .cloned()
            .collect(),
    };
    let taken = crate::app::invite::standing::take_back(server, &due).await;
    for gone in due.every() {
        let name = &gone.member.name;
        if !taken.withdrawn.contains(name) && !taken.suspended.contains(name) {
            findings.push(format!(
                "{name}'s invitation ran out and could not be taken back, so it can still \
                 be claimed"
            ));
        }
    }
}

/// Take off the record every offer seen taken up while it still stood.
///
/// Seen here because this reading holds both halves at once: what was offered, and which
/// accounts are claimed now. One taken up in time is closed so the door never judges it
/// against its offer; one claimed only after it ran out is kept, which is what keeps it
/// refused there. A rehearsal writes nothing.
fn close_taken_up(ctx: &Ctx, accounts: &[Member]) {
    if ctx.dry_run {
        return;
    }
    let offers: Offers = crate::app::record::beside(ctx, RECORD);
    let held = offers.len();
    let open = closed(offers, accounts, &ctx.hours_ago(0));
    if open.len() != held {
        crate::app::record::keep_beside(ctx, RECORD, &open);
    }
}

/// The accounts whose invitation was declined, switched off where the decline service
/// could not do it itself.
///
/// A refusal recorded against an account still switched on is one whose write did not
/// land; this reading holds the administrator's session, so it switches the account off
/// rather than leave a declined invitation claimable. One it cannot switch off is said.
async fn declined(
    ctx: &Ctx,
    server: &crate::jellyfin::Jellyfin,
    accounts: &[Member],
    findings: &mut Vec<String>,
) -> BTreeSet<String> {
    let offers: Offers = crate::app::record::beside(ctx, RECORD);
    let declined = crate::app::invite::declining::declined(ctx, &offers);
    for account in accounts {
        if declined.contains(&account.id)
            && !account.claimed
            && !account.access.disabled
            && server.suspend(&account.id).await.is_err()
        {
            findings.push(format!(
                "{} declined their invitation and the account could not be switched off, so \
                 it can still be claimed",
                account.name
            ));
        }
    }
    declined
}

/// Where this account stands, given which invitations have run out and which were
/// declined.
///
/// Declined first, because the person said so and the account is kept for that reason:
/// switched off by the decline service, or about to be. Then switched off, because it
/// overrides the rest: an account nobody can sign in to is neither a member who can nor
/// an invitation somebody could take up.
pub(super) fn standing(
    account: &Member,
    expired: &BTreeSet<String>,
    declined: &BTreeSet<String>,
) -> MemberStanding {
    if !account.claimed && declined.contains(&account.id) {
        MemberStanding::Declined
    } else if account.access.disabled {
        MemberStanding::Suspended
    } else if account.claimed {
        MemberStanding::Active
    } else if expired.contains(&account.id) {
        MemberStanding::Expired
    } else {
        MemberStanding::Invited
    }
}
