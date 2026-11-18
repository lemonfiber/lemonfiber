//! Registering a device key, and the challenge one is asked to sign.

use crate::app::Ctx;
use crate::error::{Diagnose, Problem};
use crate::instant::Instant;
use crate::journal::Kind;
use crate::keys::names_a_key;

use super::heard::{heard, Heard};
use super::refused::{
    bad_name, key_taken, name_taken, no_attestation, no_challenge, not_p256, nowhere, unproved,
};
use super::{at, changed, journalled, password, refused, standing, trusted};
use crate::device_keys::{Attestation, Challenged, PublicKey, Record, Registered};

/// A challenge for the device key holding `name` to sign, issued to `session`.
///
/// # Errors
///
/// A [`Problem`] where no active device key holds the name, or this machine supplies no
/// randomness to draw one from.
pub async fn challenged(ctx: &Ctx, name: &str, session: &str) -> Result<Challenged, Box<Problem>> {
    standing(ctx, name).await?;
    let issued = ctx
        .admitting
        .challenges
        .issued(
            ctx.seams.random.as_ref(),
            session,
            name,
            ctx.seams.clock.now(),
        )
        .await
        .ok_or_else(|| Box::new(no_challenge()))?;
    Ok(Challenged {
        challenge: issued.challenge,
        expires: Instant::of(issued.expires),
    })
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
        registered: Instant::of(now),
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
