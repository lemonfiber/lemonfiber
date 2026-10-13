//! Registering, listing and revoking device keys, and what proving with one reads.
//!
//! **Every change is heard.** A registration is journaled before it is made, a revoke as
//! soon as it is, and each raises an operator alert naming the device key, as a key's mint
//! does. A revoke is made first so a journal that cannot be written never leaves a device
//! key standing: the revoke is kept, told, and the journal's failure answered after. A
//! registration refused after the password proved the operator, and a signature that
//! does not verify, are alerted too: either is somebody holding an operator session and
//! failing to prove it with a phone.
//!
//! **Opening the record sweeps it.** Every device key registered under a password that
//! is no longer the one kept is revoked the moment the record is next opened, so a
//! password changed by any means cuts off every phone before anything is asked of one.

use std::path::{Path, PathBuf};

use crate::app::{Ctx, Outcome};
use crate::config::store::Failure;
use crate::error::{Diagnose, Problem};
use crate::journal::{Change, Kind};
use crate::keys::names_a_key;

use super::identity::{self, AppIdentity};
use super::{Attestation, Kept, Listed, Listing, PublicKey, Record, Registered, FILE};
use heard::{heard, Heard, Revoked};
use refused::{
    bad_name, key_taken, name_taken, no_attestation, not_p256, not_registered, nowhere, unproved,
};
pub use refused::{challenge_not_standing, no_challenge, signature_wrong};

/// What was asked of the device keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// Register a device key under a name, with the platform's attestation. The
    /// operator's password has proved them by the time this is asked.
    Register {
        /// What to call it.
        name: String,
        /// Its public half, as its `SubjectPublicKeyInfo` in base64url.
        public_key: String,
        /// The platform's attestation of where it lives, where one was given.
        attestation: Option<Attestation>,
    },
    /// List the device keys.
    List,
    /// Revoke the device key holding a name.
    Revoke {
        /// The device key's name.
        name: String,
    },
}

/// The operation every journal entry about a device key is written under.
pub(crate) const OPERATION: &str = "device-key";

/// Where this machine keeps its device keys, or nothing where it keeps no configuration.
#[must_use]
pub fn at(ctx: &Ctx) -> Option<PathBuf> {
    crate::app::targets::beside_env(ctx, FILE)
}

/// Carry out what was asked of the device keys.
///
/// # Errors
///
/// The [`Problem`] whichever of the three refuses with.
pub async fn asked(ctx: &Ctx, asked: Asked) -> Result<Outcome, Box<Problem>> {
    match asked {
        Asked::Register {
            name,
            public_key,
            attestation,
        } => register(ctx, &name, &public_key, attestation.as_ref())
            .await
            .map(Outcome::DeviceKey),
        Asked::List => list(ctx).await.map(Outcome::DeviceKeys),
        Asked::Revoke { name } => revoke(ctx, &name).await.map(Outcome::DeviceKeys),
    }
}

/// Register a device key, once its attestation vouches for it.
///
/// # Errors
///
/// A [`Problem`] where the name cannot name one or is taken, the public half is not a
/// P-256 key or is registered already, the attestation is missing, does not verify,
/// places the key in software or names an app this stack does not trust, or the record
/// or the journal cannot be read or written.
pub async fn register(
    ctx: &Ctx,
    name: &str,
    public_key: &str,
    attestation: Option<&Attestation>,
) -> Result<Registered, Box<Problem>> {
    if !names_a_key(name) {
        return Err(Box::new(bad_name(name)));
    }
    let key = PublicKey::read(public_key).ok_or_else(|| Box::new(not_p256()))?;
    let path = at(ctx).ok_or_else(|| Box::new(nowhere()))?;
    let under = password(ctx).ok_or_else(|| Box::new(nowhere()))?;
    let Some(attestation) = attestation else {
        return Err(refused(ctx, name, no_attestation()).await);
    };
    let trusted = trusted(ctx).map_err(|failure| Box::new(failure.problem()))?;
    let now = ctx.seams.clock.now();
    let attested = match attestation.verified(&key, &trusted, now) {
        Ok(attested) => attested,
        Err(why) => return Err(refused(ctx, name, unproved(&why)).await),
    };
    let record = Record {
        name: name.to_owned(),
        public_key: key.written(),
        fingerprint: key.fingerprint(),
        backing: attested.backing,
        app: attested.app.written(),
        registered: crate::instant::written(now).unwrap_or_default(),
        under,
        used: None,
        revoked: None,
    };
    let added = record.clone();
    changed(ctx, &path, move |ctx, kept| {
        if let Some(held) = kept.named(&added.name) {
            return Err(Box::new(name_taken(held)));
        }
        if let Some(held) = kept.holding(&added.fingerprint) {
            return Err(Box::new(key_taken(held)));
        }
        journalled(
            ctx,
            &added.name,
            Kind::DeviceKeyRegistered {
                name: added.name.clone(),
                backing: added.backing.written().to_owned(),
            },
        )?;
        kept.keys.push(added.clone());
        Ok(())
    })
    .await?;
    heard(ctx, &record, &Heard::Registered).await;
    Ok(Registered::from(&record))
}

/// Every device key this machine has registered.
///
/// # Errors
///
/// A [`Problem`] where there is nowhere device keys are kept, or the record of them
/// cannot be read or swept.
pub async fn list(ctx: &Ctx) -> Result<Listing, Box<Problem>> {
    let path = at(ctx).ok_or_else(|| Box::new(nowhere()))?;
    let kept = changed(ctx, &path, |_, kept| Ok(kept.clone())).await?;
    Ok(listed(&kept, None))
}

/// Revoke the active device key holding `name`, or say what revoking it would come to.
///
/// # Errors
///
/// A [`Problem`] where no active device key holds the name, or the record or the journal
/// cannot be read or written.
pub async fn revoke(ctx: &Ctx, name: &str) -> Result<Listing, Box<Problem>> {
    let path = at(ctx).ok_or_else(|| Box::new(nowhere()))?;
    let now = written_now(ctx);
    let (found, kept, journaled) = changed(ctx, &path, |ctx, kept| {
        let found = kept
            .active(name)
            .cloned()
            .ok_or_else(|| Box::new(not_registered(name)))?;
        let journaled = if ctx.dry_run {
            Ok(())
        } else {
            revoked(ctx, kept, std::slice::from_ref(&found), &now)
        };
        Ok((found, kept.clone(), journaled))
    })
    .await?;
    if !ctx.dry_run {
        heard(ctx, &found, &Heard::Revoked(Revoked::Asked)).await;
    }
    journaled?;
    Ok(Listing {
        rehearsed: ctx.dry_run,
        ..listed(&kept, Some(found.name))
    })
}

/// The active device key holding `name`, as the record stands once swept.
///
/// # Errors
///
/// A [`Problem`] where none does, the record cannot be read or swept, or the settings
/// naming the operator's own builds cannot be read, so whether its app is still trusted
/// cannot be said.
pub async fn standing(ctx: &Ctx, name: &str) -> Result<Record, Box<Problem>> {
    let path = at(ctx).ok_or_else(|| Box::new(nowhere()))?;
    trusted(ctx).map_err(|failure| Box::new(failure.problem()))?;
    changed(ctx, &path, |_, kept| {
        kept.active(name)
            .cloned()
            .ok_or_else(|| Box::new(not_registered(name)))
    })
    .await
}

/// Write down that the device key holding `name` proved a mint now.
///
/// # Errors
///
/// A [`Problem`] where it no longer stands, or the record cannot be written.
pub async fn used(ctx: &Ctx, name: &str) -> Result<(), Box<Problem>> {
    let path = at(ctx).ok_or_else(|| Box::new(nowhere()))?;
    let now = written_now(ctx);
    changed(ctx, &path, |_, kept| {
        let record = kept
            .keys
            .iter_mut()
            .find(|record| record.name == name && !record.is_revoked())
            .ok_or_else(|| Box::new(not_registered(name)))?;
        record.used = Some(now.clone());
        Ok(())
    })
    .await
}

/// Tell the operator that a mint carried a signature the device key holding `name` did
/// not make.
pub async fn misproved(ctx: &Ctx, record: &Record) {
    heard(ctx, record, &Heard::Misproved).await;
}

/// Revoke every device key registered under a password that is no longer the one kept,
/// or made by an app this stack no longer trusts, as the act that changed either asks.
///
/// # Errors
///
/// A [`Problem`] where the record cannot be read, swept or written.
pub async fn swept(ctx: &Ctx) -> Result<(), Box<Problem>> {
    let Some(path) = at(ctx) else {
        return Ok(());
    };
    changed(ctx, &path, |_, _| Ok(())).await
}

/// Revoke every active device key kept in the configuration directory `config`, as a
/// restore into it asks, whether or not the restore went on to land. Where no record is
/// kept there, no device key stands to revoke.
///
/// # Errors
///
/// A [`Problem`] where the record cannot be read or written, or the journal cannot be
/// written; every device key the record holds is revoked in the second case all the same.
pub async fn restored(ctx: &Ctx, config: &Path) -> Result<(), Box<Problem>> {
    let path = config.join(FILE);
    if matches!(path.try_exists(), Ok(false)) {
        return Ok(());
    }
    let now = written_now(ctx);
    let (gone, journaled) = changed(ctx, &path, |ctx, kept| {
        let gone: Vec<Record> = kept
            .keys
            .iter()
            .filter(|record| !record.is_revoked())
            .cloned()
            .collect();
        let journaled = revoked(ctx, kept, &gone, &now);
        Ok((gone, journaled))
    })
    .await?;
    for record in &gone {
        heard(ctx, record, &Heard::Revoked(Revoked::Restored)).await;
    }
    journaled
}

/// Revoke a device key as the reversal of the run that registered it.
///
/// Heard as every revoke is, and not journaled here, for the reason a key's reversal is
/// not: the reversal records what it put back as one run.
///
/// # Errors
///
/// A [`Problem`] where the record cannot be read or written.
pub(crate) async fn undone(ctx: &Ctx, name: &str) -> Result<(), Box<Problem>> {
    let path = at(ctx).ok_or_else(|| Box::new(nowhere()))?;
    let now = written_now(ctx);
    let found = changed(ctx, &path, |_, kept| {
        let found = kept.active(name).cloned();
        if found.is_some() {
            mark_revoked(kept, name, &now);
        }
        Ok(found)
    })
    .await?;
    if let Some(found) = found {
        heard(ctx, &found, &Heard::Revoked(Revoked::Asked)).await;
    }
    Ok(())
}

/// The device key an undo revokes, where it revokes one.
pub(crate) fn revoked_by(undo: &crate::journal::Undo) -> Option<&str> {
    match &undo.action {
        crate::journal::Action::RevokeDevice { name } => Some(name),
        _ => None,
    }
}

/// Every app this stack trusts now, or why the settings that name the operator's own
/// builds cannot be read.
fn trusted(ctx: &Ctx) -> Result<Vec<AppIdentity>, Failure> {
    let settings = crate::app::targets::beside_env(ctx, crate::config::paths::SETTINGS);
    identity::trusted(settings.as_deref())
}

/// The fingerprint of the password kept now, where one is.
fn password(ctx: &Ctx) -> Option<String> {
    ctx.settings
        .admission
        .as_deref()
        .and_then(crate::admission::at)
        .map(|credential| credential.fingerprint())
}

/// Make `change` to the device keys at `path`, under the hold every writer takes, once
/// every device key registered under another password or made by an app this stack no
/// longer trusts is revoked, and tell the operator of each of those after.
///
/// What `change` answers is answered; where it refuses, what it left is still written,
/// so a sweep it found is kept whatever it went on to refuse. Where the sweep's journal
/// cannot be written, the sweep is kept and told, and `change` is not made. Where the
/// settings naming the operator's own builds cannot be read, no device key is revoked
/// for its app, and none is answered as standing.
async fn changed<T>(
    ctx: &Ctx,
    path: &Path,
    mut change: impl FnMut(&Ctx, &mut Kept) -> Result<T, Box<Problem>>,
) -> Result<T, Box<Problem>> {
    let under = password(ctx);
    let apps: Result<Vec<String>, Failure> =
        trusted(ctx).map(|apps| apps.iter().map(AppIdentity::written).collect());
    let now = written_now(ctx);
    let mut swept = Vec::new();
    let mut answered = None;
    crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        swept = kept
            .keys
            .iter()
            .filter(|record| !ctx.dry_run && !record.is_revoked())
            .filter_map(|record| {
                let why = if Some(&record.under) != under.as_ref() {
                    Revoked::PasswordChanged
                } else if apps.as_ref().is_ok_and(|apps| !apps.contains(&record.app)) {
                    Revoked::AppRemoved
                } else {
                    return None;
                };
                Some((record.clone(), why))
            })
            .collect();
        let stale: Vec<Record> = swept.iter().map(|(record, _)| record.clone()).collect();
        answered = Some(revoked(ctx, kept, &stale, &now).and_then(|()| change(ctx, kept)));
        Ok(())
    })
    .map_err(|failure| Box::new(failure.problem()))?;
    for (record, why) in &swept {
        heard(ctx, record, &Heard::Revoked(*why)).await;
    }
    answered.unwrap_or_else(|| Err(Box::new(nowhere())))
}

/// Mark every one of `records` revoked at `now`, then journal each revoke, answering the
/// first journal entry that could not be written once every one was tried.
fn revoked(ctx: &Ctx, kept: &mut Kept, records: &[Record], now: &str) -> Result<(), Box<Problem>> {
    for record in records {
        mark_revoked(kept, &record.name, now);
    }
    records
        .iter()
        .map(|record| {
            journalled(
                ctx,
                &record.name,
                Kind::DeviceKeyRevoked {
                    name: record.name.clone(),
                },
            )
        })
        .fold(Ok(()), Result::and)
}

/// Mark the active device key holding `name` revoked at `now`.
fn mark_revoked(kept: &mut Kept, name: &str, now: &str) {
    for record in kept
        .keys
        .iter_mut()
        .filter(|record| record.name == name && !record.is_revoked())
    {
        record.revoked = Some(now.to_owned());
    }
}

/// The listing of `kept`.
fn listed(kept: &Kept, revoked: Option<String>) -> Listing {
    Listing {
        keys: kept.keys.iter().map(Listed::from).collect(),
        revoked,
        rehearsed: false,
    }
}

/// A refusal of a registration after the password proved the operator, heard first.
async fn refused(ctx: &Ctx, name: &str, problem: Problem) -> Box<Problem> {
    heard::refused(ctx, name, &problem).await;
    Box::new(problem)
}

/// Now, written as every instant this product writes is.
fn written_now(ctx: &Ctx) -> String {
    crate::instant::written(ctx.seams.clock.now()).unwrap_or_default()
}

/// Write a device key's registration or revoke into the journal, before it is made.
fn journalled(ctx: &Ctx, name: &str, kind: Kind) -> Result<(), Box<Problem>> {
    let paths = crate::app::targets::layout(ctx)
        .ok_or_else(|| Box::new(crate::config::store::Failure::Nowhere.problem()))?;
    let change = Change {
        at: ctx.stamp(),
        operation: OPERATION.to_owned(),
        target: name.to_owned(),
        kind,
    };
    crate::app::recover::journalled(&paths.journal(), &[change], ctx.seams.random.as_ref())
        .map_err(|failure| Box::new(failure.problem()))
}

mod heard;
mod refused;

#[cfg(test)]
mod tests;
