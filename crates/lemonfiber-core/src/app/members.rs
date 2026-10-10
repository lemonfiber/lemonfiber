//! The household a member signs in against, as the stack holds it at that moment, and
//! whether it vouches for whoever holds an account.
//!
//! The web surface checks a member's name and password with the media server, and
//! asks it again on every call whether that member's account still stands. Both
//! questions go to the household the stack holds when they are asked: a stack seeded
//! after the surface started, or one whose admin password was minted again, is
//! answered by what it holds now rather than by what it held at start.

use std::sync::Arc;

use super::{record, Ctx};
use crate::invitation::{lapsed_unseen, Offers, RECORD};
use crate::ports::service::Household;

/// The media server's household, opened from what the stack holds now.
///
/// Nothing where the stack's manifest cannot be read, where it runs no media server,
/// or where no admin password is recorded for it yet. Each of those is a stack with
/// no household to ask, and a member is refused rather than admitted on it.
pub async fn household(ctx: &Ctx) -> Option<Arc<dyn Household>> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    let identity: Arc<dyn Household> = super::targets::identity(ctx, &manifest).await?;
    Some(identity)
}

/// Whether the household vouches for whoever holds this account now.
///
/// It does unless the account was offered as an invitation and the offer ran out before
/// anybody was seen to take it up: an account claimed after that is one whoever found it
/// first could have set a password on, and signing in to it proves only that. A member
/// signing in while their offer still stands has taken it up, so the offer is closed here
/// and they are not judged against it again.
///
/// Read from what this program recorded offering, so it holds whether or not anything has
/// swept the household since.
#[must_use]
pub fn vouched_for(ctx: &Ctx, id: &str) -> bool {
    let now = ctx.hours_ago(0);
    let mut offers: Offers = record::beside(ctx, RECORD);
    if lapsed_unseen(&offers, id, &now) {
        return false;
    }
    if offers.remove(id).is_some() {
        record::keep_beside(ctx, RECORD, &offers);
    }
    true
}

/// Whether `token` claims the offer standing on account `id` now: its hash is the one the
/// offer carries, the offer has not lapsed, and it was not declined.
pub async fn offers_claim(ctx: &Ctx, id: &str, token: &str) -> bool {
    let offers: Offers = record::beside(ctx, RECORD);
    let presented = Some(lemonfiber_sidecar::TokenHash::of(token));
    let carried = offers.get(id).is_some_and(|offer| offer.claim == presented);
    carried
        && !lapsed_unseen(&offers, id, &ctx.hours_ago(0))
        && !super::invite::declining::declined(ctx, &offers)
            .await
            .contains(id)
}

/// Take the claim off the offer standing on account `id`, so its token claims nothing
/// again.
pub fn claim_spent(ctx: &Ctx, id: &str) {
    let mut offers: Offers = record::beside(ctx, RECORD);
    if offers
        .get_mut(id)
        .and_then(|offer| offer.claim.take())
        .is_some()
    {
        record::keep_beside(ctx, RECORD, &offers);
    }
}

#[cfg(test)]
mod tests;
