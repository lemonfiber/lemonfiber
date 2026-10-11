//! Minting a key as a claim, collecting it with that claim, and revoking one nobody
//! collected in time.

use std::path::Path;

use crate::app::Ctx;
use crate::error::Problem;
use crate::journal::Kind;

use super::super::claiming::{Claim, Claimed, KEY_LINK, MINUTES_TO_COLLECT};
use super::super::{Kept, Minted, Minter, Record, Secret};
use super::refused::{claim_not_open, no_secret, nowhere, unkept, unlinkable};
use super::{added, at, changed, heard, journalled, prepared, Heard, Prepared};

/// Mint a key as a claim, and answer with the link that collects it.
///
/// Refused before anything is minted where no link can be written, because a claim
/// another machine cannot reach collects nothing.
///
/// # Errors
///
/// A [`Problem`] wherever a mint of a secret would refuse; where the stack has not been
/// served where another machine reaches it; and where this machine will not supply the
/// randomness a claim is made of.
pub(super) async fn mint(
    ctx: &Ctx,
    name: &str,
    scope: &str,
    purpose: &str,
    by: &Minter,
) -> Result<Claimed, Box<Problem>> {
    let Prepared {
        path,
        scope,
        purpose,
    } = prepared(ctx, name, scope, purpose, by).await?;
    let material = crate::companion::paired(ctx)
        .await
        .map_err(|problem| Box::new(unlinkable(&problem)))?
        .material;
    let claim = Claim::mint(ctx.seams.random.as_ref()).ok_or_else(|| Box::new(no_secret()))?;
    let now = ctx.seams.clock.now();
    let lapsing = now
        .checked_add(std::time::Duration::from_secs(
            u64::try_from(MINUTES_TO_COLLECT * 60).unwrap_or_default(),
        ))
        .unwrap_or(now);
    let lapses = crate::instant::written(lapsing).unwrap_or_default();
    let expires = lapsing
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let minted = crate::instant::written(now).unwrap_or_default();
    let record = Record::claimed(name, scope, purpose, &claim, lapses, minted, by.clone());
    changed(ctx, &path, |kept| added(ctx, &path, kept, &record, by)).await?;
    heard(ctx, &record, Heard::Minted).await;
    Ok(Claimed {
        link: crate::companion::linked(KEY_LINK, &material, expires, &[("claim", claim.as_str())]),
        lapses: record
            .pending
            .as_ref()
            .map(|pending| pending.lapses.clone())
            .unwrap_or_default(),
        name: record.name,
        scope: record.scope.written(),
        purpose,
    })
}

/// Collect the key `offered` claims, minting its secret now, and answer with it.
///
/// # Errors
///
/// A [`Problem`] where no key waits on that claim, it was collected already or it has
/// lapsed, all said the same way; where there is nowhere keys are kept or they cannot be
/// read or written; and where this machine will not supply a secret's randomness.
pub(super) async fn collect(
    ctx: &Ctx,
    offered: &str,
    from: Option<String>,
) -> Result<Minted, Box<Problem>> {
    let path = at(ctx).ok_or_else(|| Box::new(nowhere()))?;
    let (record, secret) = changed(ctx, &path, |kept| collected(ctx, &path, kept, offered)).await?;
    heard(ctx, &record, Heard::Collected { from }).await;
    let reached = crate::companion::reaching(ctx).await;
    Ok(Minted {
        name: record.name,
        scope: record.scope.written(),
        purpose: record.purpose,
        secret,
        address: reached.address,
        pin: reached.pin,
        caution: reached.caution,
    })
}

/// The key `offered` claims, collected in `kept` with a secret minted now, and the
/// secret.
///
/// # Errors
///
/// A [`Problem`] where no key waits on that claim, it was collected already or it has
/// lapsed, all said the same way; where the journal or the keys cannot be written; and
/// where this machine will not supply a secret's randomness.
fn collected(
    ctx: &Ctx,
    path: &Path,
    kept: &mut Kept,
    offered: &str,
) -> Result<(Record, Secret), Box<Problem>> {
    let now = ctx.seams.clock.now();
    let found = kept.awaiting(offered).filter(|&at| {
        kept.keys
            .get(at)
            .and_then(|record| record.pending.as_ref())
            .is_some_and(|pending| !pending.lapsed(now))
    });
    let Some(record) = found.and_then(|at| kept.keys.get_mut(at)) else {
        return Err(Box::new(claim_not_open()));
    };
    let secret = Secret::mint(ctx.seams.random.as_ref()).ok_or_else(|| Box::new(no_secret()))?;
    journalled(
        ctx,
        record,
        Kind::KeyCollected {
            name: record.name.clone(),
            scope: record.scope.written(),
        },
    )?;
    record.collected(&secret);
    let collected = record.clone();
    kept.keep(path).map_err(|why| Box::new(unkept(why)))?;
    Ok((collected, secret))
}

/// Revoke every key whose claim lapsed with nobody having collected it, each journaled,
/// and answer with them for the caller to have heard once it lets the keys go.
///
/// A rehearsal revokes nothing: it writes nothing anywhere.
///
/// # Errors
///
/// A [`Problem`] where the journal or the keys cannot be written.
pub(super) fn lapse(ctx: &Ctx, path: &Path, kept: &mut Kept) -> Result<Vec<Record>, Box<Problem>> {
    if ctx.dry_run {
        return Ok(Vec::new());
    }
    let now = ctx.seams.clock.now();
    let lapsed: Vec<Record> = kept
        .keys
        .iter()
        .filter(|record| {
            record.is_waiting()
                && record
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.lapsed(now))
        })
        .cloned()
        .collect();
    if lapsed.is_empty() {
        return Ok(lapsed);
    }
    let written = crate::instant::written(now).unwrap_or_default();
    for record in &lapsed {
        journalled(
            ctx,
            record,
            Kind::KeyRevoked {
                name: record.name.clone(),
                scope: record.scope.written(),
            },
        )?;
        for kept in kept
            .keys
            .iter_mut()
            .filter(|kept| kept.name == record.name && kept.is_waiting())
        {
            kept.revoked = Some(written.clone());
        }
    }
    kept.keep(path).map_err(|why| Box::new(unkept(why)))?;
    Ok(lapsed)
}

#[cfg(test)]
mod tests;
