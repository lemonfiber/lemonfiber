//! What the media server already holds where an invitation is going, and what has run out.
//!
//! Apart from the errand above because it is about the accounts already there rather than
//! the one being offered. Both halves of the offer turn on it: whether there is already an
//! account under this name decides whether the run is making one or dating an existing one
//! again, and which accounts have run past their window decides what the run sweeps away
//! on its way through.

use crate::app::Ctx;
use crate::invitation::{offered, run_out, Offered, HOURS_OF_RECORD, HOURS_TO_CLAIM};
use crate::model::InvitationStanding;
use crate::ports::service::{Household as _, Member};

/// What was found where the invitation was going.
pub(crate) fn standing_of(already: Option<&Member>) -> InvitationStanding {
    match already {
        Some(member) if member.claimed => InvitationStanding::Joined,
        // Unclaimed, but somebody has been in it: their password was taken off rather
        // than an offer they never took up. Told apart because the message differs —
        // nobody is being invited, and what they need to hear is that a password they
        // had has stopped working.
        Some(member) if member.last_seen.is_some() => InvitationStanding::Reset,
        Some(_) => InvitationStanding::Waiting,
        None => InvitationStanding::Made,
    }
}

/// The account this invitation is for, where the household already holds one.
///
/// **Matched without regard to case**, because the media server refuses a second
/// account whose name differs from an existing one only in case — so a match missed
/// here walks straight into the refusal this exists to prevent, and the operator is
/// handed the server's own word for it, which is `400`.
///
/// **The ones that have run out are counted too**, and that is the whole of offering
/// an expired invitation again without making somebody a second time. The account is
/// already theirs; what has run out is the window on it, and a window is restarted by
/// dating the invitation again rather than by building a new account to carry it. Left
/// out, this run would withdraw the account — which is to say delete it — and then make
/// another under the same name with a different identifier, so anything already linked
/// to them would be linked to somebody who no longer exists.
pub(crate) fn already_here<'a>(held: &'a Held, name: &str) -> Option<&'a Member> {
    let asked = name.to_lowercase();
    held.household
        .iter()
        .find(|member| member.name.to_lowercase() == asked)
}

/// Whether this account is one the sweep was about to take back.
pub(crate) fn has_run_out(held: &Held, member: &Member) -> bool {
    held.spent.iter().any(|gone| gone.member.id == member.id)
}

/// What the media server holds right now, as this command needs to see it.
pub(crate) struct Held {
    /// Every account it has, claimed or not.
    pub(crate) household: Vec<Member>,
    /// The invitations among them that have run out.
    pub(crate) spent: Vec<Offered>,
}

/// The invitations nobody claimed in time, as the media server holds them now.
///
/// Best-effort: a media server that will not answer is not a reason to refuse the
/// invitation the operator asked for. The sweep runs again next time.
///
/// **Reading and acting are separate** so that a rehearsal can do the first without
/// the second — the whole of what `--dry-run` promises is that the second does not
/// happen, and a sweep that removed accounts on the way to saying what it would do
/// would be the flag doing the damage it exists to prevent.
pub(crate) async fn held(ctx: &Ctx, server: &crate::jellyfin::Jellyfin) -> Held {
    let cutoff = ctx.hours_ago(HOURS_TO_CLAIM);
    let since = ctx.hours_ago(HOURS_OF_RECORD);
    let (Ok(household), Ok(records)) =
        (server.household().await, server.when_invited(&since).await)
    else {
        return Held {
            household: Vec::new(),
            spent: Vec::new(),
        };
    };
    let waiting = offered(household.clone(), &records);
    Held {
        household,
        spent: run_out(&waiting, &cutoff).into_iter().cloned().collect(),
    }
}

/// Take back the invitations that have run out, naming the ones actually taken.
///
/// A server that refuses one is not reported as having given it back: the operator
/// reads this list as what is gone.
pub(crate) async fn take_back(
    server: &crate::jellyfin::Jellyfin,
    spent: &[Offered],
) -> Vec<String> {
    let mut taken = Vec::new();
    for invitation in spent {
        if server.withdraw(&invitation.member.id).await.is_ok() {
            taken.push(invitation.member.name.clone());
        }
    }
    taken
}
