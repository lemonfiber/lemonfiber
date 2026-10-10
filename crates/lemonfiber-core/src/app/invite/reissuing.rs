//! Making an account claimable again, once somebody has lost the way into it.
//!
//! Apart from the offer itself because it answers a different question. [`super`] decides
//! who has an account; this decides that somebody who has one may set its password again —
//! and it reaches for the same message, because after a reset the thing to send *is* an
//! invitation.
//!
//! **The operator cannot learn what is chosen next.** The call carries a flag rather than
//! a value, so there is nowhere to put a password even in error, and the account goes back
//! to having none at all until whoever holds it sets one at the media server.

use crate::app::Ctx;
use crate::invitation::HOURS_TO_CLAIM;
use crate::model::{Invitation, InvitationStanding, Linked};
use crate::ports::service::Household as _;

use super::standing::Held;
use super::{reaching, Reaching};

/// Make somebody's account claimable again, and hand back the invitation to send them.
///
/// **A reset here is not a password chosen for somebody.** The account goes back to
/// having none at all — the state an invitation leaves it in — so whoever holds it sets
/// the next first password themselves, at the media server, where the operator cannot
/// read it. The call that does it carries a flag and not a password, so there is nowhere
/// to put one even in error.
///
/// What comes back is an [`Invitation`] rather than a report of its own, because after
/// this the thing to send *is* an invitation: the same address, the same code, the same
/// line about setting a password. A second shape here would be a second account of one
/// message, and the two would drift.
///
/// # Errors
///
/// Returns a [`Problem`](crate::error::Problem) where the stack has no media server,
/// where it will not answer, where nobody is named, where nobody by that name is here,
/// where the account named administers the server, or where the reset could not be
/// dated or the account switched back on.
pub(crate) async fn reissue(
    ctx: &Ctx,
    name: String,
) -> Result<Invitation, Box<crate::error::Problem>> {
    let name = name.trim().to_owned();
    let Reaching {
        server,
        reachable,
        manifest,
    } = reaching(ctx, &name).await?;

    let Ok(household) = server.household().await else {
        return Err(Box::new(unreadable()));
    };
    let asked = name.to_lowercase();
    let Some(member) = household
        .iter()
        .find(|member| member.name.to_lowercase() == asked)
        .cloned()
    else {
        return Err(Box::new(nobody_here(&name)));
    };
    // Refused for the same reason a removal refuses it: this is the account the program
    // signs in as, and taking its password away would leave nothing to sign in with.
    if member.access.administrator {
        return Err(Box::new(runs_the_server(&member.name)));
    }

    if ctx.dry_run {
        return Ok(renewed(member.name, reachable, true));
    }
    // Dated before the password comes off, so an account is never left claimable with
    // nothing to say when its window closes — which the next sweep would read as closed.
    let held = Held {
        offers: crate::app::record::beside(ctx, crate::invitation::RECORD),
        household,
        spent: crate::invitation::Spent::default(),
    };
    if !super::offering::recorded_now(ctx, &held, &member) {
        return Err(Box::new(super::offering::unrecorded(&member.name)));
    }
    if server.unclaim(&member.id).await.is_err() {
        return Err(Box::new(would_not_reissue(&member.name)));
    }
    // Switched back on after the password comes off, and never before: an account locked
    // by wrong guesses, or switched off when its last window closed, is one its person
    // cannot sign in to until this is written — and switched on first, it would open to
    // the old password for as long as the reset took.
    super::offering::guarded(&server, &member, None, false).await?;
    // A new decline address with the new offer: the old token went with the old record,
    // so a refusal of it no longer reads as this account's standing.
    let decline = super::declining::issued(ctx, &manifest.services, &held.household, &member).await;
    let joining = super::joining::joining(ctx, &member, InvitationStanding::Reset).await;
    Ok(Invitation {
        decline,
        join: joining.join,
        unjoinable: joining.unjoinable,
        ..renewed(member.name, reachable, false)
    })
}
/// The invitation a reissue account is sent with.
///
/// `Reset` rather than `Made`, because what the person needs to hear is different: nobody
/// is being invited, and the news is that the password they had has stopped working. The
/// window is the offer's, and it is real — at the end of it the sweep switches this account
/// off, keeping it for another reissue, rather than removing what somebody has watched on.
/// That is why the message says what happens at the end of it rather than leaving the
/// word "lapses" to carry it.
fn renewed(name: String, reachable: crate::door::Address, rehearsed: bool) -> Invitation {
    Invitation {
        name,
        address: reachable.url,
        decline: None,
        join: None,
        unjoinable: None,
        caution: reachable.caution,
        hours: HOURS_TO_CLAIM,
        withdrawn: Vec::new(),
        suspended: Vec::new(),
        rehearsed,
        standing: InvitationStanding::Reset,
        // Whoever it is was already known to the request service, or was never known to
        // it; taking a password away changes neither.
        linked: Linked::NotTried,
        // A reset takes a password off an account whose access somebody already chose,
        // and it changes none of it. Saying what that access is would be reporting a
        // setting this run did not make, on a message about a password.
        applied: None,
    }
}
/// Said where the media server will not say who holds an account.
fn unreadable() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::codes::reissue::UNREADABLE,
        "the media server would not say who holds an account, so nothing was reset",
        "Making an account claimable again starts by finding it, and that read did not \
         answer",
        crate::error::Remedy::new("Check the media server is running, then run this again"),
    )
}
/// Said where nobody by that name is in the household.
fn nobody_here(name: &str) -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::codes::reissue::NOBODY_HERE,
        format!("nobody called {name} is in this household"),
        "Nothing was reset. The name has to match an account the media server holds, \
         though not its capitalisation",
        crate::error::Remedy::new("Run `lemonfiber household` to see who is here"),
    )
}
/// Said where the account named administers the server.
fn runs_the_server(name: &str) -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::codes::reissue::RUNS_THE_SERVER,
        format!("{name} administers the media server, so its password is not one to reset"),
        "This is the account lemonfiber signs in as, and taking its password away would \
         leave nothing to sign in with",
        crate::error::Remedy::new(
            "Reset a household member instead; to change the administrator's own \
             password, do it in the media server's settings",
        ),
    )
}
/// Said where the media server refused to make the account claimable again.
fn would_not_reissue(name: &str) -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::codes::reissue::WOULD_NOT_REISSUE,
        format!("the media server would not reset {name}'s password, so nothing changed"),
        "Their existing password still works and the account is untouched",
        crate::error::Remedy::new("Check the media server is running, then run this again"),
    )
}
