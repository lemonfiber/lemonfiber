//! The household's own commands, each answered with what it came to: somebody offered an
//! account or taken out of the household, a request decided, the requests nobody rules on
//! swept, and the household read.

use super::command::{Arranged, Decision, Inviting, Whom};
use super::{expiring, invite, remove, Ctx, Outcome};
use crate::asking::run as asking;
use crate::error::Problem;
use crate::household::run as household;

/// The household as it stands, or one member of it.
pub(super) async fn household(ctx: &Ctx, member: Option<&Whom>) -> Result<Outcome, Box<Problem>> {
    household::household(ctx, member)
        .await
        .map(Outcome::Household)
}

/// Remove somebody from the household, or say what that would cost.
pub(super) async fn removed(
    ctx: &Ctx,
    name: String,
    offer: Option<String>,
) -> Result<Outcome, Box<Problem>> {
    remove::remove(ctx, name, offer.as_deref())
        .await
        .map(Outcome::Removal)
}

/// Offer somebody an account: the one household request whose parts are taken apart.
pub(super) async fn invited(ctx: &Ctx, inviting: Inviting) -> Result<Outcome, Box<Problem>> {
    let Inviting {
        name,
        allowance,
        confirm,
    } = inviting;
    invite::offer(ctx, name, allowance, confirm)
        .await
        .map(Outcome::Invitation)
}

/// What became of one thing the household asked for.
pub(super) async fn decided(ctx: &Ctx, decision: Decision) -> Result<Outcome, Box<Problem>> {
    asking::deciding(ctx, &decision)
        .await
        .map(Outcome::Household)
}

/// Held on until the arrangement changes under it, sweeping as it goes.
///
/// How often it wakes is this command's own, because a period in whole days has no moment
/// to miss — and a surface that could choose it could choose one that misses the day. It
/// is also the one household command that acts while nobody is watching, which is why its
/// period is named rather than defaulted: naming one records it and stops.
pub(super) async fn sweeping(ctx: &Ctx, arranged: Arranged) -> Result<Outcome, Box<Problem>> {
    expiring::expiring(ctx, arranged, expiring::SWEEPING)
        .await
        .map(Outcome::Household)
}
