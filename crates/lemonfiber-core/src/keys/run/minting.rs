//! Minting a key: preparing it, keeping it, and hearing it.

use std::path::{Path, PathBuf};

use super::{
    at, bad_name, heard, journalled, members_may_mint, members_may_not_mint, name_held, name_taken,
    names_a_key, no_secret, not_a_purpose, not_a_scope, not_for_yourself, nowhere, resolved,
    unkept, Ctx, Heard, Held, Kept, Kind, Minted, Minter, Opened, Problem, Purpose, Record, Scope,
    Secret, Wanted,
};

/// Mint a key, and answer with its secret and what a client elsewhere needs beside it.
///
/// # Errors
///
/// A [`Problem`] where the name, the scope or the purpose names nothing, the name is
/// taken, a member's account cannot be found, a member asked for anything but a key of
/// their own, the keys or the journal cannot be read or written, or this machine will
/// not supply the randomness a secret is made of.
pub async fn mint(
    ctx: &Ctx,
    name: &str,
    scope: &str,
    purpose: &str,
    by: &Minter,
    proved_by: Option<&str>,
) -> Result<Minted, Box<Problem>> {
    let Prepared {
        path,
        scope,
        purpose,
    } = prepared(ctx, name, scope, purpose, by).await?;
    let secret = Secret::mint(ctx.seams.random.as_ref()).ok_or_else(|| Box::new(no_secret()))?;
    let record =
        Record::minted(name, scope, purpose, &secret, ctx.stamp(), by.clone()).proved_by(proved_by);
    let mut opened = Opened::at(ctx, &path).await?;
    let made = added(ctx, &path, &mut opened.kept, &record, by);
    opened.let_go(ctx).await;
    made?;
    heard(ctx, &record, Heard::Minted).await;
    let reached = crate::companion::reaching(ctx).await;
    Ok(Minted {
        name: record.name,
        scope: record.scope.written(),
        purpose,
        secret,
        address: reached.address,
        pin: reached.pin,
        caution: reached.caution,
    })
}

/// What a mint has settled before the keys are held: where they are, and the scope and
/// purpose asked for, each read and allowed.
pub(super) struct Prepared {
    /// Where the keys are kept.
    pub(super) path: PathBuf,
    /// What the key will admit.
    pub(super) scope: Scope,
    /// What it is for.
    pub(super) purpose: Purpose,
}

/// Everything a mint, of a secret or of a claim, checks before it holds the keys.
///
/// # Errors
///
/// A [`Problem`] where the name, the scope or the purpose names nothing, a member's
/// account cannot be found, a member asked for anything but a key of their own or is not
/// allowed keys, or there is nowhere keys are kept.
pub(super) async fn prepared(
    ctx: &Ctx,
    name: &str,
    scope: &str,
    purpose: &str,
    by: &Minter,
) -> Result<Prepared, Box<Problem>> {
    if matches!(by, Minter::Member { .. }) && !members_may_mint(ctx) {
        return Err(Box::new(members_may_not_mint()));
    }
    if !names_a_key(name) {
        return Err(Box::new(bad_name(name)));
    }
    let wanted = Wanted::read(scope).ok_or_else(|| Box::new(not_a_scope(scope)))?;
    let purpose = Purpose::read(purpose).ok_or_else(|| Box::new(not_a_purpose(purpose)))?;
    let path = at(ctx).ok_or_else(|| Box::new(nowhere()))?;
    let scope = resolved(ctx, wanted).await?;
    if let Minter::Member { id } = by {
        if scope.member() != Some(id.as_str()) {
            return Err(Box::new(not_for_yourself()));
        }
    }
    Ok(Prepared {
        path,
        scope,
        purpose,
    })
}

/// Add `record` to the keys, journaled first, where its name is nobody's yet.
///
/// # Errors
///
/// A [`Problem`] where the name is taken, or the journal or the keys cannot be written.
pub(super) fn added(
    ctx: &Ctx,
    path: &Path,
    kept: &mut Kept,
    record: &Record,
    by: &Minter,
) -> Result<(), Box<Problem>> {
    if let Some(held) = kept.named(&record.name) {
        return Err(Box::new(match by {
            Minter::Member { id } if held.scope.member() != Some(id.as_str()) => {
                name_held(&record.name)
            }
            _ => name_taken(held),
        }));
    }
    journalled(
        ctx,
        record,
        Kind::KeyMinted {
            name: record.name.clone(),
            scope: record.scope.written(),
        },
    )?;
    kept.keys.push(record.clone());
    kept.keep(path).map_err(|why| Box::new(unkept(why)))
}

/// The hold on the keys at `path`, or why it could not be taken.
pub(super) async fn held(path: &Path) -> Result<Held, Box<Problem>> {
    Held::taken(path).await.map_err(|why| Box::new(unkept(why)))
}
