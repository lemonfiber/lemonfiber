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
/// or where the account named administers the server.
pub(crate) async fn reissued(
    ctx: &Ctx,
    name: String,
) -> Result<Invitation, Box<crate::error::Problem>> {
    let name = name.trim().to_owned();
    let Reaching {
        server, reachable, ..
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
        return Ok(reissue(member.name, reachable, true));
    }
    if server.unclaim(&member.id).await.is_err() {
        return Err(Box::new(would_not_reissue(&member.name)));
    }
    Ok(reissue(member.name, reachable, false))
}
/// The invitation a reissued account is sent with.
///
/// `Reset` rather than `Made`, because what the person needs to hear is different: nobody
/// is being invited, and the news is that the password they had has stopped working. The
/// window is the offer's, and it is real — the sweep withdraws this one like any other,
/// which for an account somebody has watched on is a larger loss than for an offer nobody
/// took up. That is why the message says what happens at the end of it rather than
/// leaving the word "lapses" to carry it.
fn reissue(name: String, reachable: crate::door::Address, rehearsed: bool) -> Invitation {
    Invitation {
        name,
        address: reachable.url,
        caution: reachable.caution,
        hours: HOURS_TO_CLAIM,
        withdrawn: Vec::new(),
        rehearsed,
        standing: InvitationStanding::Reset,
        // Whoever it is was already known to the request service, or was never known to
        // it; taking a password away changes neither.
        linked: Linked::NotTried,
    }
}
/// Said where the media server will not say who holds an account.
fn unreadable() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("REISSUE-1"),
        crate::error::Severity::Error,
        "the media server would not say who holds an account, so nothing was reset",
        "Making an account claimable again starts by finding it, and that read did not \
         answer",
        crate::error::Remedy::new("Check the media server is running, then run this again"),
    )
}
/// Said where nobody by that name is in the household.
fn nobody_here(name: &str) -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("REISSUE-2"),
        crate::error::Severity::Error,
        format!("nobody called {name} is in this household"),
        "Nothing was reset. The name has to match an account the media server holds, \
         though not its capitalisation",
        crate::error::Remedy::new("Run `lemonfiber household` to see who is here"),
    )
}
/// Said where the account named administers the server.
fn runs_the_server(name: &str) -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("REISSUE-3"),
        crate::error::Severity::Error,
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
        crate::error::Code::new("REISSUE-4"),
        crate::error::Severity::Error,
        format!("the media server would not reset {name}'s password, so nothing changed"),
        "Their existing password still works and the account is untouched",
        crate::error::Remedy::new("Check the media server is running, then run this again"),
    )
}
