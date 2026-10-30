//! The decline service's media server key, as the inventory lists it, prints it and
//! replaces it.
//!
//! It is held by the decline service, and asked of it only over `invite.decline`: the
//! service answers the key's SHA-256 alone, and the key itself is the core's own copy, the
//! one filed under the service's name in the media server that hashes to it.
//!
//! **A replacement keeps a working key at every moment.** A new key is minted, handed to
//! the service, proven by the SHA-256 it answers and against the media server, and only
//! then is the old one revoked. Where any step fails the new key is revoked and the old one
//! handed back. A revocation of the old key that the media server refuses leaves it filed
//! under the service's name and held by nothing, which the next seed revokes.

use std::path::Path;
use std::sync::Arc;

use lemonfiber_manifest::Service;
use lemonfiber_sidecar::decline::Key;

use super::rotating::{said, unproven, would_rotate};
use crate::app::invite::declining;
use crate::app::Ctx;
use crate::app_keys::DECLINE_APP;
use crate::credential::{Held, Origin, Propagation, Reach, Rotation, State};
use crate::ports::service::{AppKeys, Declining, KeyHeld};
use crate::wiring::Fillers;

/// What the key is recorded as.
pub(super) const SETTING: &str = "decline/jellyfin.key";

/// What the inventory calls it.
const NAME: &str = "Media server decline key";

/// Where it lives.
const LOCATION: &str = "held by the decline service";

/// The one thing that authenticates with it.
const CONSUMER: &str = "the decline service, which switches off an invitation the invitee refuses";

/// What is said where the key is not there.
const ABSENT: &str = "Media server decline key is not there yet, so the decline service cannot \
                      switch off an invitation anybody refuses. Run lemonfiber seed.";

/// What a rehearsal says a replacement would take.
const ROTATING: &str = "a real run would mint a new key on the media server, hand it to the \
                        decline service, prove the service holds it and the media server takes \
                        it, and only then revoke the old one. Nothing was minted here, and \
                        nothing was handed over.";

/// What a landed replacement says.
const LANDED: &str = "The media server took the new key, and the decline service holds it";

/// What is said where the service cannot be asked, or may not be handed the key.
const UNASKED: &str = "the decline service cannot be asked over its contract, or is not one the \
                       media server's key may be handed to, so nothing was minted";

/// Why there is no key to replace.
const UNREPLACEABLE: &str = "the decline service did not answer holding a key the media server \
                             lists, so there is nothing to replace and nothing was minted; run \
                             lemonfiber seed";

/// Why there is nothing to mint with.
pub(super) const NO_ADMINISTRATOR: &str =
    "lemonfiber holds no administrator for this media server, so there is nothing to mint \
     a key with; run lemonfiber seed";

/// Why a new key was revoked before it was ever used.
const UNWRITTEN: &str =
    "the new key could not be handed to the decline service, so it was revoked again";

/// Why a new key was revoked once the media server refused it.
pub(super) const UNTAKEN: &str =
    "The media server did not take the new key, so it was revoked and the old one put back";

/// Why a new key was revoked once the service did not hold it.
const UNHELD: &str = "the decline service did not answer holding the new key, so it was revoked \
                      and the old one handed back";

/// The key's line in the inventory, where the stack runs the decline service: there where
/// the service answers holding one.
///
/// Read off the service alone, so listing what the stack holds asks the media server
/// nothing; the key itself is only read where the operator asks to see it.
pub(super) async fn held(
    ctx: &Ctx,
    services: &[Service],
    fillers: &Fillers,
    project: Option<&Path>,
) -> Option<Held> {
    declining::service(services)?;
    let holding = match declining::asked(ctx, fillers, project?).await {
        Some(decline) => decline.held().await.is_ok_and(|held| held.is_some()),
        None => false,
    };
    Some(Held {
        name: NAME.to_owned(),
        setting: SETTING.to_owned(),
        consumers: vec![CONSUMER.to_owned()],
        location: LOCATION.to_owned(),
        origin: Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state: if holding {
            State::Active
        } else {
            State::Absent
        },
        fingerprint: None,
        advisory: (!holding).then(|| ABSENT.to_owned()),
    })
}

/// The core's own copy of the key the decline service holds.
pub(super) async fn value(ctx: &Ctx, fillers: &Fillers, project: Option<&Path>) -> Option<String> {
    let decline = declining::asked(ctx, fillers, project?).await?;
    let holding = decline.held().await.ok()??;
    let server = crate::app::targets::declined_server(fillers)?;
    let client = server.administering(ctx).await?;
    let filed = client.filed_as(DECLINE_APP).await.ok()?;
    crate::seed::run::decline::named_by(&filed, &holding.key)
}

/// Replace the key, keeping a working one at every moment.
pub(super) async fn rotate(
    ctx: &Ctx,
    held: &Held,
    fillers: &Fillers,
    project: Option<&Path>,
) -> Rotation {
    let Some(server) = crate::app::targets::declined_server(fillers) else {
        return unproven(held, NO_ADMINISTRATOR);
    };
    let Some(client) = server.administering(ctx).await else {
        return unproven(held, NO_ADMINISTRATOR);
    };
    if ctx.dry_run {
        return would_rotate(held, ROTATING);
    }
    let Some(decline) = asked(ctx, fillers, (&server.filler, project)).await else {
        return unproven(held, UNASKED);
    };
    let holding = decline.held().await.ok().flatten();
    let filed = client.filed_as(DECLINE_APP).await.unwrap_or_default();
    let Some((old, KeyHeld { pins, .. })) = holding.and_then(|holding| {
        crate::seed::run::decline::named_by(&filed, &holding.key).map(|old| (old, holding))
    }) else {
        return unproven(held, UNREPLACEABLE);
    };
    let minted = match client.mint(DECLINE_APP).await {
        Ok(minted) => minted,
        Err(failure) => return unproven(held, &said(&failure)),
    };
    let Some(key) = Key::read(&minted).ok() else {
        let _ = client.revoke(&minted).await;
        return unproven(held, UNWRITTEN);
    };
    if decline.hold(&minted, &pins).await.is_err() {
        let _ = client.revoke(&minted).await;
        return unproven(held, UNWRITTEN);
    }
    let holds = decline
        .held()
        .await
        .is_ok_and(|holding| holding.is_some_and(|holding| holding.key == key.fingerprint()));
    if !holds {
        put_back(decline.as_ref(), client.as_ref(), (&old, &pins), &minted).await;
        return unproven(held, UNHELD);
    }
    if client.answers_to(&minted).await.is_err() {
        put_back(decline.as_ref(), client.as_ref(), (&old, &pins), &minted).await;
        return unproven(held, UNTAKEN);
    }
    for other in &filed {
        let _ = client.revoke(other).await;
    }
    Rotation::landed(
        &held.name,
        LANDED,
        vec![Propagation {
            consumer: CONSUMER.to_owned(),
            reach: Reach::Updated,
        }],
    )
}

/// The decline service under `project`, where it can be asked and may be handed a key
/// the media server `server` holds.
async fn asked(
    ctx: &Ctx,
    fillers: &Fillers,
    (server, project): (&crate::wiring::Filler, Option<&Path>),
) -> Option<Arc<dyn Declining>> {
    declining::may_hold(fillers, server).then_some(())?;
    declining::asked(ctx, fillers, project?).await
}

/// Hand the service `old` again, with the certificates it accepted, and revoke `minted`.
async fn put_back(
    decline: &dyn Declining,
    client: &dyn AppKeys,
    (old, pins): (&str, &[String]),
    minted: &str,
) {
    let _ = decline.hold(old, pins).await;
    let _ = client.revoke(minted).await;
}

#[cfg(test)]
mod tests;
