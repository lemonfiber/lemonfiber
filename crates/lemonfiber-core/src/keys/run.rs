//! Minting, listing and revoking keys, as every surface asks for them.
//!
//! **Every mint and every revoke is heard.** Each is journaled before it is made, so a
//! key that exists is a key the record names, and each raises an operator alert naming
//! the key and its scope, so a key nobody meant to mint is seen.
//!
//! **What a member may do is decided here**, beside the operator's, rather than at the
//! door a request came through: a member mints and revokes only keys scoped to
//! themselves, whatever surface asked.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::alert::{Alert, Moment};
use crate::app::{Ctx, Outcome};
use crate::error::{Diagnose, Problem, Severity};
use crate::journal::{Change, Kind};
use crate::PRODUCT;

use super::listing::PURPOSES;
use super::{
    names_a_key, Kept, Listed, Listing, Minted, Minter, Purpose, Record, Scope, Secret, State,
    Used, Wanted, FILE, USED_FILE,
};
use refused::{
    bad_name, members_may_not_mint, name_held, name_taken, no_secret, no_such_key, no_such_member,
    not_a_purpose, not_a_scope, not_for_yourself, nowhere, unasked, unkept,
};

/// What was asked of the keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// Mint a key under a name, with one scope and a purpose.
    Mint {
        /// What to call it.
        name: String,
        /// What it admits, as `read`, `act` or `member:<account>`.
        scope: String,
        /// What it is for, as `home-assistant`, `mcp` or `other`.
        purpose: String,
        /// Who is minting it.
        by: Minter,
    },
    /// List the keys, without their secrets: every one to the operator, and to a
    /// member only those scoped to them.
    List {
        /// Who is asking.
        by: Minter,
    },
    /// Revoke the key holding a name.
    Revoke {
        /// The key's name.
        name: String,
        /// Who is revoking it.
        by: Minter,
    },
}

/// The operation every journal entry about a key is written under.
pub(crate) const OPERATION: &str = "key";

/// What the alerts about keys are filed under, ahead of the key's name.
const CHECK: &str = "key.";

/// The kind every alert about a key shares.
const ALERT_KIND: &str = "integration-key";

/// Where this machine keeps its keys, or nothing where it keeps no configuration.
#[must_use]
pub fn at(ctx: &Ctx) -> Option<PathBuf> {
    crate::app::targets::beside_env(ctx, FILE)
}

/// Where the serving surface writes each key's last use.
#[must_use]
pub fn used_at(ctx: &Ctx) -> Option<PathBuf> {
    crate::app::targets::beside_env(ctx, USED_FILE)
}

/// Carry out what was asked of the keys.
///
/// # Errors
///
/// The [`Problem`] whichever of the three refuses with.
pub async fn asked(ctx: &Ctx, asked: Asked) -> Result<Outcome, Box<Problem>> {
    match asked {
        Asked::Mint {
            name,
            scope,
            purpose,
            by,
        } => mint(ctx, &name, &scope, &purpose, &by)
            .await
            .map(Outcome::Minted),
        Asked::List { by } => list(ctx, &by).await.map(Outcome::Keys),
        Asked::Revoke { name, by } => revoke(ctx, &name, &by).await.map(Outcome::Keys),
    }
}

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
) -> Result<Minted, Box<Problem>> {
    if matches!(by, Minter::Member { .. }) && !members_may_mint(ctx) {
        return Err(Box::new(members_may_not_mint()));
    }
    if !names_a_key(name) {
        return Err(Box::new(bad_name(name)));
    }
    let wanted = Wanted::read(scope).ok_or_else(|| Box::new(not_a_scope(scope)))?;
    let purpose = Purpose::read(purpose).ok_or_else(|| Box::new(not_a_purpose(purpose)))?;
    let path = at(ctx).ok_or_else(|| Box::new(nowhere()))?;
    let mut kept = read(&path)?;
    if let Some(held) = kept.named(name) {
        return Err(Box::new(match by {
            Minter::Member { id } if held.scope.member() != Some(id.as_str()) => name_held(name),
            _ => name_taken(held),
        }));
    }
    let scope = resolved(ctx, wanted).await?;
    if let Minter::Member { id } = by {
        if scope.member() != Some(id.as_str()) {
            return Err(Box::new(not_for_yourself()));
        }
    }
    let secret = Secret::mint(ctx.seams.random.as_ref()).ok_or_else(|| Box::new(no_secret()))?;
    let now = crate::instant::written(ctx.seams.clock.now()).unwrap_or_default();
    let record = Record::minted(name, scope, purpose, &secret, now, by.clone());
    journalled(
        ctx,
        &record,
        Kind::KeyMinted {
            name: record.name.clone(),
            scope: record.scope.written(),
        },
    )?;
    kept.keys.push(record.clone());
    kept.keep(&path).map_err(|why| Box::new(unkept(why)))?;
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

/// The keys `by` may see, without their secrets: every one to the operator, and to a
/// member only those scoped to them.
///
/// # Errors
///
/// A [`Problem`] where there is nowhere keys are kept, or the record of them is there and
/// cannot be read.
pub async fn list(ctx: &Ctx, by: &Minter) -> Result<Listing, Box<Problem>> {
    let path = at(ctx).ok_or_else(|| Box::new(nowhere()))?;
    let kept = read(&path)?;
    Ok(listed(ctx, &seen_by(kept, by), None).await)
}

/// What of `kept` is `by`'s to see.
///
/// A member is shown only keys scoped to them, so another member's keys, their names
/// included, are nothing a member's listing, mint or revoke can tell apart from keys
/// that do not exist.
fn seen_by(kept: Kept, by: &Minter) -> Kept {
    match by {
        Minter::Operator => kept,
        Minter::Member { id } => Kept {
            keys: kept
                .keys
                .into_iter()
                .filter(|record| record.scope.member() == Some(id.as_str()))
                .collect(),
        },
    }
}

/// Whether the operator has allowed household members to mint keys for themselves.
///
/// Off unless the setting reads as on, and off where there is no configuration to read
/// it from.
#[must_use]
pub fn members_may_mint(ctx: &Ctx) -> bool {
    ctx.settings
        .env_file
        .as_deref()
        .and_then(|path| crate::config::store::read(path).ok())
        .and_then(|file| file.get(crate::config::MEMBER_KEYS_KEY).map(str::to_owned))
        .is_some_and(|value| crate::config::reads_as_on(&value))
}

/// Revoke the key holding `name`, or say what revoking it would come to.
///
/// # Errors
///
/// A [`Problem`] where no key holds the name or it is already revoked, a member asked to
/// revoke a key that is not theirs, or the keys or the journal cannot be read or written.
pub async fn revoke(ctx: &Ctx, name: &str, by: &Minter) -> Result<Listing, Box<Problem>> {
    let path = at(ctx).ok_or_else(|| Box::new(nowhere()))?;
    let mut kept = read(&path)?;
    let seen = seen_by(kept.clone(), by);
    let Some(found) = active(&seen, name) else {
        return Err(Box::new(no_such_key(name, seen.named(name))));
    };
    if ctx.dry_run {
        return Ok(listed(ctx, &seen, Some(name.to_owned())).await);
    }
    journalled(
        ctx,
        &found,
        Kind::KeyRevoked {
            name: found.name.clone(),
            scope: found.scope.written(),
        },
    )?;
    revoked(ctx, &path, &mut kept, &found.name)?;
    heard(ctx, &found, Heard::Revoked).await;
    Ok(listed(ctx, &seen_by(kept, by), Some(found.name)).await)
}

/// Revoke a key as the reversal of the run that minted it.
///
/// Heard as every revoke is. Not journaled here: the reversal records what it put back
/// as one run, and a second entry for the same revoke would be a change made twice.
/// A key already revoked, or gone, is what was asked for.
///
/// # Errors
///
/// A [`Problem`] where the keys cannot be read or written.
pub(crate) async fn undone(ctx: &Ctx, name: &str) -> Result<(), Box<Problem>> {
    let path = at(ctx).ok_or_else(|| Box::new(nowhere()))?;
    let mut kept = read(&path)?;
    let Some(found) = active(&kept, name) else {
        return Ok(());
    };
    revoked(ctx, &path, &mut kept, &found.name)?;
    heard(ctx, &found, Heard::Revoked).await;
    Ok(())
}

/// The key an undo revokes, where it revokes one.
pub(crate) fn revoked_by(undo: &crate::journal::Undo) -> Option<&str> {
    match &undo.action {
        crate::journal::Action::Revoke { name } => Some(name),
        _ => None,
    }
}

/// The keys kept at `path`, or why they could not be read.
fn read(path: &Path) -> Result<Kept, Box<Problem>> {
    Kept::at(path).map_err(|why| Box::new(unkept(why)))
}

/// The active key holding `name`, where one does.
fn active(kept: &Kept, name: &str) -> Option<Record> {
    kept.keys
        .iter()
        .find(|record| record.name == name && !record.is_revoked())
        .cloned()
}

/// Mark the active key holding `name` revoked now, and keep the record.
fn revoked(ctx: &Ctx, path: &Path, kept: &mut Kept, name: &str) -> Result<(), Box<Problem>> {
    let now = crate::instant::written(ctx.seams.clock.now()).unwrap_or_default();
    for record in kept
        .keys
        .iter_mut()
        .filter(|record| record.name == name && !record.is_revoked())
    {
        record.revoked = Some(now.clone());
    }
    kept.keep(path).map_err(|why| Box::new(unkept(why)))
}

/// The scope a request asked for, with a member's account found in the household.
async fn resolved(ctx: &Ctx, wanted: Wanted) -> Result<Scope, Box<Problem>> {
    let account = match wanted {
        Wanted::Read => return Ok(Scope::Read),
        Wanted::Act => return Ok(Scope::Act),
        Wanted::Member(account) => account,
    };
    let household = crate::app::members::household(ctx).ok_or_else(|| Box::new(unasked()))?;
    let members = household
        .household()
        .await
        .map_err(|_| Box::new(unasked()))?;
    members
        .into_iter()
        .find(|member| member.id == account || member.name.eq_ignore_ascii_case(&account))
        .map(|member| Scope::Member {
            id: member.id,
            name: member.name,
        })
        .ok_or_else(|| Box::new(no_such_member(&account)))
}

/// The listing of `kept`, each member's key standing as the household now says.
async fn listed(ctx: &Ctx, kept: &Kept, revoked: Option<String>) -> Listing {
    let used = used_at(ctx).map(|path| Used::at(&path)).unwrap_or_default();
    let members = if kept
        .keys
        .iter()
        .any(|record| record.scope.member().is_some())
    {
        standing(ctx).await
    } else {
        None
    };
    Listing {
        keys: kept
            .keys
            .iter()
            .map(|record| Listed {
                name: record.name.clone(),
                scope: record.scope.written(),
                purpose: record.purpose,
                state: state(record, members.as_ref()),
                minted: record.minted.clone(),
                used: used.at.get(&record.name).cloned(),
                revoked: record.revoked.clone(),
                member_minted: matches!(record.by, Minter::Member { .. }),
            })
            .collect(),
        revoked,
        purposes: PURPOSES.to_owned(),
        rehearsed: false,
    }
}

/// The ids of everybody the household holds now, or nothing where it could not be asked.
async fn standing(ctx: &Ctx) -> Option<BTreeSet<String>> {
    let household = crate::app::members::household(ctx)?;
    let members = household.household().await.ok()?;
    Some(members.into_iter().map(|member| member.id).collect())
}

/// Where one key stands, given who the household holds.
fn state(record: &Record, members: Option<&BTreeSet<String>>) -> State {
    if record.is_revoked() {
        return State::Revoked;
    }
    match (record.scope.member(), members) {
        (None, _) => State::Active,
        (Some(_), None) => State::Unconfirmed,
        (Some(id), Some(members)) if members.contains(id) => State::Active,
        (Some(_), Some(_)) => State::Orphaned,
    }
}

/// Write a key's mint or revoke into the journal, before it is made.
fn journalled(ctx: &Ctx, record: &Record, kind: Kind) -> Result<(), Box<Problem>> {
    let paths = crate::app::targets::layout(ctx)
        .ok_or_else(|| Box::new(crate::config::store::Failure::Nowhere.problem()))?;
    let change = Change {
        at: ctx.stamp(),
        operation: OPERATION.to_owned(),
        target: record.name.clone(),
        kind,
    };
    crate::app::recover::journalled(&paths.journal(), &[change], ctx.seams.random.as_ref())
        .map_err(|failure| Box::new(failure.problem()))
}

/// Which of the two a key's alert is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Heard {
    /// It was minted.
    Minted,
    /// It was revoked.
    Revoked,
}

/// Tell the operator, now and in the record of alerts, that a key was minted or revoked.
async fn heard(ctx: &Ctx, record: &Record, heard: Heard) {
    let alert = alert(record, heard);
    let mut outbox = crate::app::outbox::load(ctx);
    outbox.owe(vec![alert.clone()]);
    ctx.narrator.say(&alert.summary).await;
    // Delivered because the narrator cannot fail: the operator has the sentence and the
    // outbox the history of its having been given.
    outbox.delivered(&|_| 0);
    crate::app::outbox::save(ctx, &outbox);
}

/// The alert a key's mint or revoke raises, naming the key and its scope.
fn alert(record: &Record, heard: Heard) -> Alert {
    let scope = record.scope.written();
    let (summary, meaning, remedy) = match heard {
        Heard::Minted => (
            format!(
                "A key named {} was minted, with the scope {scope}",
                record.name
            ),
            format!(
                "Whoever holds it can reach {PRODUCT} as far as {scope} admits, from now until \
                 it is revoked."
            ),
            format!(
                "If nobody meant to mint it, revoke it: {PRODUCT} key revoke {}",
                record.name
            ),
        ),
        Heard::Revoked => (
            format!(
                "The key named {} was revoked, with the scope {scope}",
                record.name
            ),
            "Whatever held it is refused from its next request.".to_owned(),
            "Mint a new key for anything that still needs one".to_owned(),
        ),
    };
    let check = format!("{CHECK}{}", record.name);
    Alert {
        check: check.clone(),
        kind: ALERT_KIND.to_owned(),
        moment: Moment::Onset,
        severity: Severity::Warning,
        summary,
        meaning,
        remedies: vec![remedy],
        affected: vec![check],
        exit: None,
    }
}

mod refused;

#[cfg(test)]
mod tests;
