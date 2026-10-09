//! Writing down that an account was offered, and making the account one that can be claimed.
//!
//! Apart from the errand because these are the two writes every offer ends in, whichever
//! way it came: a new account, an expired one offered again, and a reset. **Both happen
//! before the offer is handed back**, and a failure at either refuses it. The message the
//! operator is about to send promises a window and an account somebody can sign in to, and
//! an offer this program cannot date is one the next sweep treats as already run out.

use crate::app::Ctx;
use crate::invitation::{recorded, Offer, HOURS_TO_CLAIM, RECORD};
use crate::ports::service::{Allowed, Member};

use super::standing::Held;

/// Write down that this account was offered now, and when that runs out.
///
/// Whether it could be is the answer, and the caller says what that means: for a new
/// account it means no offer, and for one offered again it means a window not restarted.
/// A record this program cannot read is not one it writes over.
#[must_use]
pub(super) fn recorded_now(ctx: &Ctx, held: &Held, member: &Member) -> bool {
    let offer = Offer {
        offered: ctx.hours_ago(0),
        lapses: ctx.hours_ago(-HOURS_TO_CLAIM),
        decline: None,
    };
    let offers = recorded(held.offers.clone(), &held.household, &member.id, offer);
    crate::app::record::keep(
        crate::app::targets::beside_env(ctx, RECORD).as_deref(),
        &offers,
    )
    .is_ok()
}

/// Make the account one its person can claim, bounded against guessing, and narrowed to
/// what was chosen.
///
/// # Errors
///
/// Where the media server will not write it: said as a refusal to narrow where anything
/// was chosen, since that is what the operator asked for, and otherwise as a refusal to
/// ready the account. `new` says which account a refusal to narrow leaves behind.
pub(super) async fn guarded(
    server: &dyn crate::ports::service::Household,
    member: &Member,
    allowed: Option<&Allowed>,
    new: bool,
) -> Result<(), Box<crate::error::Problem>> {
    let nothing = Allowed::default();
    if server
        .claimable(&member.id, allowed.unwrap_or(&nothing))
        .await
        .is_ok()
    {
        return Ok(());
    }
    Err(Box::new(match allowed {
        Some(_) => super::allowing::would_not_allow(&member.name, new),
        None => unguarded(&member.name),
    }))
}

/// Said where the offer could not be written down.
pub(super) fn unrecorded(name: &str) -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::codes::invite::UNRECORDED,
        format!(
            "when {name} was offered an account could not be written down, so it was not offered"
        ),
        "An invitation runs out a set time after it is offered, and one this machine has no \
         date for is taken back the next time anybody is invited",
        crate::error::Remedy::new(
            "Check the configuration directory can be written, then run this again",
        ),
    )
}

/// Said where the account could not be made one its person can claim.
pub(super) fn unguarded(name: &str) -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::codes::invite::UNGUARDED,
        format!("the media server would not ready {name}'s account to be claimed"),
        "An account is offered switched on, with a limit on wrong passwords, and the media \
         server would not write either",
        crate::error::Remedy::new("Check the media server is running, then run this again"),
    )
}
