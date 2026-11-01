//! The decline address an invitation carries, and the invitations the decline service
//! acts on.
//!
//! Where the stack runs the decline service, every offer gets a token of its own. The
//! token goes out once, in the decline address, and this program keeps only its hash:
//! in the offer record, and in the invitations it hands the service over
//! `invite.decline`. Offering the same person again mints a new token, so an older
//! address stops declining anything.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lemonfiber_contract::capabilities::invite::decline;
use lemonfiber_sidecar::TokenHash;

use crate::app::targets::{spoken, Spoken};
use crate::app::Ctx;
use crate::invitation::Offers;
use crate::ports::service::{
    Claimed, Declinable, Declined, Declining, Invitations, Lapsed, Member, TakenBack,
};
use crate::wiring::Fillers;

/// The decline service's id in the stack manifest.
pub(crate) const SERVICE: &str = "decline";

/// How many random bytes back an offer's decline or claim token: 128 bits, held only as
/// a hash.
const TOKEN_BYTES: usize = 16;

/// The decline service, where the stack runs one.
pub(crate) fn service(
    services: &[lemonfiber_manifest::Service],
) -> Option<&lemonfiber_manifest::Service> {
    services.iter().find(|service| service.id == SERVICE)
}

/// A new token for one offer, or none where the randomness could not be had.
pub(super) fn minted(ctx: &Ctx) -> Option<String> {
    ctx.seams
        .random
        .bytes(TOKEN_BYTES)
        .map(|bytes| crate::secret::render(&bytes))
}

/// The invitations the decline service may act on, from the offers still out.
///
/// An offer is in them where it carries a token and its account is still an unclaimed
/// member of the household: a claimed account is no invitation, and one that is gone
/// has nothing to decline.
pub(super) fn invitations(offers: &Offers, household: &[Member]) -> Invitations {
    Invitations {
        // The pinned media server says whether an account has a password.
        claimed: Claimed::HasPassword,
        invitations: offers
            .iter()
            .filter_map(|(id, offer)| {
                let member = household
                    .iter()
                    .find(|member| &member.id == id && !member.claimed)?;
                Some(Declinable {
                    token: offer.decline.as_ref()?.as_str().to_owned(),
                    account: id.clone(),
                    name: member.name.clone(),
                    issued: seconds(&offer.offered)?,
                    lapses: seconds(&offer.lapses)?,
                })
            })
            .collect(),
    }
}

/// The decline service as the core asks it: over `invite.decline`, where the service
/// filling it speaks it.
///
/// Nothing where nothing fills it, where its filler speaks nothing, or where it speaks
/// the contract and cannot be asked over it.
pub(crate) async fn asked(ctx: &Ctx, fillers: &Fillers) -> Option<Arc<dyn Declining>> {
    let (filler, _) = fillers.filling(decline::CAPABILITY)?;
    match spoken(ctx, filler, decline::CAPABILITY, decline::MAJOR).await {
        Spoken::Over(adapter) => Some(Arc::new(decline::Adapter(adapter))),
        Spoken::Unanswered | Spoken::Not => None,
    }
}

/// Whether the service filling `invite.decline` may be handed a key the media server
/// `server` holds: the key administers the whole server, so it crosses only where any of
/// that server's credentials may.
pub(crate) fn may_hold(fillers: &Fillers, server: &crate::wiring::Filler) -> bool {
    fillers
        .filling(decline::CAPABILITY)
        .is_some_and(|(filler, _)| crate::wiring::crosses(server.holder(), filler.holder()))
}

/// The accounts whose standing offer was declined: the ones whose offer's token the
/// decline service answers a refusal for.
///
/// Matched on the token rather than the account, so an account offered again since it
/// was declined, under a new token, is an invitation again rather than still declined.
/// A service that cannot be asked, or does not answer, declines nobody.
pub(crate) async fn declined(ctx: &Ctx, offers: &Offers) -> BTreeSet<String> {
    let Some(declining) = here(ctx).await else {
        return BTreeSet::new();
    };
    declining
        .refusals()
        .await
        .map_or_else(|_| BTreeSet::new(), |refusals| refused(offers, &refusals))
}

/// The decline service of the stack written to disk, as the core asks it.
async fn here(ctx: &Ctx) -> Option<Arc<dyn Declining>> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    asked(ctx, &crate::app::targets::fillers_here(ctx, &manifest)).await
}

/// Whether `offer`'s token is the one `hash` names.
fn tokened(offer: &crate::invitation::Offer, hash: &str) -> bool {
    offer
        .decline
        .as_ref()
        .is_some_and(|token| token.as_str() == hash)
}

/// The accounts among `offers` whose token `refusals` holds a refusal of.
fn refused(offers: &Offers, refusals: &[Declined]) -> BTreeSet<String> {
    offers
        .iter()
        .filter(|(_, offer)| refusals.iter().any(|one| tokened(offer, &one.token)))
        .map(|(id, _)| id.clone())
        .collect()
}

/// The accounts the decline service removed when their offer's window closed, each
/// against the name it was made under.
///
/// Matched on the offer's token, as a refusal is, so a removal recorded against an
/// earlier offer of the same account says nothing about the one standing now. A service
/// that cannot be asked, or does not answer, removed nobody.
pub(crate) async fn removed_at_lapse(ctx: &Ctx, offers: &Offers) -> BTreeMap<String, String> {
    let Some(declining) = here(ctx).await else {
        return BTreeMap::new();
    };
    declining
        .lapses()
        .await
        .map_or_else(|_| BTreeMap::new(), |lapses| removed(offers, &lapses))
}

/// The accounts among `offers` whose token `lapses` holds the removal of.
fn removed(offers: &Offers, lapses: &[Lapsed]) -> BTreeMap<String, String> {
    offers
        .iter()
        .filter_map(|(id, offer)| {
            let lapse = lapses.iter().find(|one| tokened(offer, &one.token))?;
            (lapse.outcome == TakenBack::Removed && &lapse.account == id)
                .then(|| (id.clone(), lapse.name.clone()))
        })
        .collect()
}

/// The offers with `token`'s hash on `member`'s, which must already be recorded.
pub(super) fn with_token(mut offers: Offers, member: &str, token: &str) -> Option<Offers> {
    offers.get_mut(member)?.decline = Some(TokenHash::of(token));
    Some(offers)
}

/// The decline address for `token`: the household's address for the service's port, and
/// the page the service says it declines an invitation on.
pub(super) async fn address(
    ctx: &Ctx,
    service: &lemonfiber_manifest::Service,
    declining: &dyn Declining,
    token: &str,
) -> Option<String> {
    let page = declining.page().await.ok()?;
    let page = page.strip_prefix('/').filter(|page| page.contains(TOKEN))?;
    let reachable = super::household_address(ctx, service.port?).await?;
    Some(format!(
        "{}/{}",
        reachable.url.trim_end_matches('/'),
        page.replace(TOKEN, token)
    ))
}

/// Where a decline page's path holds the token.
const TOKEN: &str = "{token}";

/// Give the offer just recorded for `member` a decline token, and say where it declines.
///
/// Nothing where the stack runs no decline service, where the person is already in the
/// household, or where any step could not be taken: the token is recorded on the offer
/// first, then the table is written, then the address is made, and an invitation goes
/// out without a decline address rather than with one that declines nothing.
pub(super) async fn issued(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    household: &[Member],
    member: &Member,
) -> Option<String> {
    let service = service(&manifest.services)?;
    let declining = asked(ctx, &crate::app::targets::fillers_here(ctx, manifest)).await?;
    let token = minted(ctx)?;
    let offers: Offers = crate::app::record::beside(ctx, crate::invitation::RECORD);
    let offers = with_token(offers, &member.id, &token)?;
    crate::app::record::keep(
        crate::app::targets::beside_env(ctx, crate::invitation::RECORD).as_deref(),
        &offers,
    )
    .ok()?;
    let mut household = household.to_vec();
    if !household.iter().any(|held| held.id == member.id) {
        household.push(member.clone());
    }
    declining
        .apply_invitations(&invitations(&offers, &household))
        .await
        .ok()?;
    address(ctx, service, declining.as_ref(), &token).await
}

/// The seconds since the Unix epoch a recorded moment names.
pub(super) fn seconds(moment: &str) -> Option<u64> {
    let at: jiff::Timestamp = moment.parse().ok()?;
    u64::try_from(at.as_second()).ok()
}

#[cfg(test)]
mod tests;
