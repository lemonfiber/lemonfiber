//! The decline address an invitation carries, and the table the decline service acts on.
//!
//! Where the stack runs the decline service, every offer gets a token of its own. The
//! token goes out once, in the decline address, and this program keeps only its hash:
//! in the offer record, and in the table it writes into the service's configuration
//! directory. Offering the same person again mints a new token, so an older address
//! stops declining anything.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use lemonfiber_sidecar::decline::{File, Invitation, Refusals, Table, TokenHash};

use crate::app::Ctx;
use crate::invitation::Offers;
use crate::ports::service::Member;

/// The decline service's id in the stack manifest.
pub(super) const SERVICE: &str = "decline";

/// How many random bytes back a decline token: 128 bits, held only as a hash.
const TOKEN_BYTES: usize = 16;

/// The decline service, where the stack runs one.
pub(super) fn service(
    services: &[lemonfiber_manifest::Service],
) -> Option<&lemonfiber_manifest::Service> {
    services.iter().find(|service| service.id == SERVICE)
}

/// A new decline token, or none where the randomness could not be had.
pub(super) fn minted(ctx: &Ctx) -> Option<String> {
    ctx.seams
        .random
        .bytes(TOKEN_BYTES)
        .map(|bytes| crate::secret::render(&bytes))
}

/// The table the decline service reads, from the offers still out.
///
/// An offer is in it where it carries a token and its account is still an unclaimed
/// member of the household: a claimed account is no invitation, and one that is gone
/// has nothing to decline.
pub(super) fn table(offers: &Offers, household: &[Member]) -> Table {
    Table::of(
        offers
            .iter()
            .filter_map(|(id, offer)| {
                let member = household
                    .iter()
                    .find(|member| &member.id == id && !member.claimed)?;
                Some(Invitation {
                    token: offer.decline.clone()?,
                    account: id.clone(),
                    name: member.name.clone(),
                    issued: seconds(&offer.offered)?,
                    lapses: seconds(&offer.lapses)?,
                })
            })
            .collect(),
    )
}

/// Where the table goes: the decline service's configuration directory, under the
/// project root the stack's config volumes are mounted from.
pub(super) fn table_path(project: &Path) -> PathBuf {
    project
        .join("config")
        .join(SERVICE)
        .join(File::Table.name())
}

/// The accounts whose standing offer was declined: the ones whose offer's token the
/// decline service recorded a refusal for.
///
/// Matched on the token rather than the account, so an account offered again since it
/// was declined, under a new token, is an invitation again rather than still declined.
/// A record the service has not written, or one that cannot be read, declines nobody.
pub(crate) fn declined(ctx: &Ctx, offers: &Offers) -> BTreeSet<String> {
    crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref())
        .and_then(|project| std::fs::read_to_string(refusals_path(&project)).ok())
        .and_then(|text| Refusals::read(&text).ok())
        .map_or_else(BTreeSet::new, |refusals| refused(offers, &refusals))
}

/// The accounts among `offers` whose token `refusals` records a refusal of.
fn refused(offers: &Offers, refusals: &Refusals) -> BTreeSet<String> {
    offers
        .iter()
        .filter(|(_, offer)| {
            offer
                .decline
                .as_ref()
                .is_some_and(|token| refusals.of(token).is_some())
        })
        .map(|(id, _)| id.clone())
        .collect()
}

/// Where the decline service records its refusals, beside the table.
pub(super) fn refusals_path(project: &Path) -> PathBuf {
    project
        .join("config")
        .join(SERVICE)
        .join(File::Refusals.name())
}

/// The offers with `token`'s hash on `member`'s, which must already be recorded.
pub(super) fn with_token(mut offers: Offers, member: &str, token: &str) -> Option<Offers> {
    offers.get_mut(member)?.decline = Some(TokenHash::of(token));
    Some(offers)
}

/// The decline address for `token`, on the household's address for the service's port.
pub(super) async fn address(
    ctx: &Ctx,
    service: &lemonfiber_manifest::Service,
    token: &str,
) -> Option<String> {
    let reachable = super::household_address(ctx, service.port?).await?;
    Some(format!(
        "{}/decline/{token}",
        reachable.url.trim_end_matches('/')
    ))
}

/// Give the offer just recorded for `member` a decline token, and say where it declines.
///
/// Nothing where the stack runs no decline service, where the person is already in the
/// household, or where any step could not be taken: the token is recorded on the offer
/// first, then the table is written, then the address is made, and an invitation goes
/// out without a decline address rather than with one that declines nothing.
pub(super) async fn issued(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    household: &[Member],
    member: &Member,
) -> Option<String> {
    let service = service(services)?;
    let project =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref())?;
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
    crate::config::store::write(&table_path(&project), &table(&offers, &household).written())
        .ok()?;
    address(ctx, service, &token).await
}

/// The seconds since the Unix epoch a recorded moment names.
fn seconds(moment: &str) -> Option<u64> {
    let at: jiff::Timestamp = moment.parse().ok()?;
    u64::try_from(at.as_second()).ok()
}

#[cfg(test)]
mod tests;
