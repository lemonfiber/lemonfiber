//! The request gate's own media server key, as the inventory lists it, prints it and
//! replaces it.
//!
//! It is held by the gate on its media server route, and asked of it only over
//! `request.gate`: the gate answers the key's SHA-256 alone, and the key itself is the
//! core's own copy, the one filed under the gate's name in the media server that hashes to
//! it.
//!
//! **A replacement keeps a working key at every moment.** A new key is minted, handed to
//! the gate with every route it admits, proven by the SHA-256 the gate answers and against
//! the media server, and only then is the old one revoked. Where any step fails the new key
//! is revoked and the old routes handed back. A revocation of the old key that the media
//! server refuses leaves it filed under the gate's name and held by nothing, which the next
//! seed revokes.

use std::path::Path;
use std::sync::Arc;

use lemonfiber_manifest::Service;

use super::declining::{NO_ADMINISTRATOR, UNTAKEN};
use super::rotating::{said, unproven, would_rotate};
use crate::app::gating;
use crate::app::targets::MediaServer;
use crate::app::Ctx;
use crate::app_keys::GATE_APP;
use crate::credential::{Held, Origin, Propagation, Reach, Rotation, State};
use crate::ports::digest::sha256;
use crate::ports::service::{Admitted, AppKeys, Gated, Gating, Target};
use crate::wiring::Fillers;

/// What the key is recorded as.
pub(super) const SETTING: &str = "request-gate/upstreams.json#jellyfin";

/// What the inventory calls it.
const NAME: &str = "Jellyfin request-gate key";

/// Where it lives.
const LOCATION: &str = "held by the request gate on its media server route";

/// The one thing that authenticates with it.
const CONSUMER: &str =
    "the request gate, which reads the media server's libraries and members for the request service";

/// What is said where the key is not there.
const ABSENT: &str = "The media server's request-gate key is not there yet, so the request \
                      gate cannot read the media server for the request service. Run \
                      lemonfiber seed.";

/// What is said where the gate cannot be asked, or may not be handed the key.
const UNASKED: &str = "the request gate cannot be asked over its contract, or is not one the \
                       media server's key may be handed to, so nothing was minted";

/// What a rehearsal says a replacement would take.
const ROTATING: &str = "a real run would mint a new key on the media server, hand it to the \
                        request gate with every route it admits, prove the gate holds it and \
                        the media server takes it, and only then revoke the old one. Nothing \
                        was minted here, and nothing was handed over.";

/// What a landed replacement says.
const LANDED: &str = "The media server took the new key, and the request gate holds it";

/// Why a new key was revoked before it was ever used.
const UNWRITTEN: &str =
    "the new key could not be handed to the request gate, so it was revoked again";

/// Why a new key was revoked once the gate did not answer holding it.
const UNHELD: &str = "the request gate did not answer holding the new key, so it was revoked \
                      and the old routes handed back";

/// The key's line in the inventory, where the stack runs the gate beside a media server
/// it answers for: there where the gate answers presenting one on its media server route.
///
/// Read off the gate alone, so listing what the stack holds asks the media server nothing;
/// the key itself is only read where the operator asks to see it.
pub(super) async fn held(
    ctx: &Ctx,
    services: &[Service],
    fillers: &Fillers,
    project: Option<&Path>,
) -> Option<Held> {
    gating::service(services)?;
    MediaServer::of(fillers)?.gate_kind()?;
    let gate = gating::asked(ctx, fillers, project?).await;
    let presented = match gate {
        Some(gate) => gate.admitted().await.is_ok_and(|admitted| {
            admitted
                .iter()
                .any(|route| matches!(route.target, Target::MediaServer { .. }))
        }),
        None => false,
    };
    Some(Held {
        name: NAME.to_owned(),
        setting: SETTING.to_owned(),
        consumers: vec![CONSUMER.to_owned()],
        location: LOCATION.to_owned(),
        origin: Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state: if presented {
            State::Active
        } else {
            State::Absent
        },
        fingerprint: None,
        advisory: (!presented).then(|| ABSENT.to_owned()),
    })
}

/// The core's own copy of the key the gate presents to the media server.
pub(super) async fn value(ctx: &Ctx, fillers: &Fillers, project: Option<&Path>) -> Option<String> {
    let gate = gating::asked(ctx, fillers, project?).await?;
    let admitted = gate.admitted().await.ok()?;
    let client = MediaServer::of(fillers)?.administering(ctx).await?;
    let filed = client.filed_as(GATE_APP).await.ok()?;
    crate::seed::run::gate::held(&admitted, &filed)
}

/// Replace the key, keeping a working one at every moment.
pub(super) async fn rotate(
    ctx: &Ctx,
    held: &Held,
    fillers: &Fillers,
    project: Option<&Path>,
) -> Rotation {
    let Some(server) = MediaServer::of(fillers).filter(|server| server.gate_kind().is_some())
    else {
        return unproven(held, NO_ADMINISTRATOR);
    };
    let Some(client) = server.administering(ctx).await else {
        return unproven(held, NO_ADMINISTRATOR);
    };
    if ctx.dry_run {
        return would_rotate(held, ROTATING);
    }
    let Some(gate) = asked(ctx, fillers, (&server, project)).await else {
        return unproven(held, UNASKED);
    };
    let admitted = match gate.admitted().await {
        Ok(admitted) => admitted,
        Err(failure) => return unproven(held, &said(&failure)),
    };
    let filed = client.filed_as(GATE_APP).await.unwrap_or_default();
    let Some(old) = crate::seed::run::gate::held(&admitted, &filed) else {
        return unproven(held, ABSENT);
    };
    let before = match restated(ctx, fillers, &admitted, &old) {
        Ok(before) => before,
        Err(why) => return unproven(held, &why),
    };
    let minted = match client.mint(GATE_APP).await {
        Ok(minted) => minted,
        Err(failure) => return unproven(held, &said(&failure)),
    };
    let after = presenting(&before, &minted);
    let handed = minted.split_whitespace().count() == 1 && gate.admit(&after).await.is_ok();
    if !handed {
        let _ = client.revoke(&minted).await;
        return unproven(held, UNWRITTEN);
    }
    if !holds(gate.as_ref(), &minted).await {
        put_back(gate.as_ref(), client.as_ref(), &before, &minted).await;
        return unproven(held, UNHELD);
    }
    if client.answers_to(&minted).await.is_err() {
        put_back(gate.as_ref(), client.as_ref(), &before, &minted).await;
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

/// The gate under `project`, where it can be asked and may be handed a key `server`
/// holds.
async fn asked(
    ctx: &Ctx,
    fillers: &Fillers,
    (server, project): (&MediaServer, Option<&Path>),
) -> Option<Arc<dyn Gating>> {
    gating::may_hold(fillers, server.filler.holder()).then_some(())?;
    gating::asked(ctx, fillers, project?).await
}

/// Every route of `admitted` as it is applied, each presenting the credential the core
/// holds for it: `key` on the media server's, and the key minted for each curator on its.
///
/// # Errors
///
/// The route whose credential the core holds no copy of.
fn restated(
    ctx: &Ctx,
    fillers: &Fillers,
    admitted: &[Admitted],
    key: &str,
) -> Result<Vec<Gated>, String> {
    let curators = crate::seed::run::gate::curator_routes(ctx, fillers);
    admitted
        .iter()
        .map(|route| {
            let credential = match route.target {
                Target::MediaServer { .. } => Some(key),
                Target::Curator { .. } => curators
                    .iter()
                    .find(|curator| {
                        curator.path == route.path
                            && sha256(&curator.credential) == route.credential
                    })
                    .map(|curator| curator.credential.as_str()),
            }
            .ok_or_else(|| {
                format!(
                    "lemonfiber holds no copy of the credential the request gate presents on its \
                     route {}, so nothing was minted",
                    route.path
                )
            })?;
            Ok(Gated {
                target: route.target.clone(),
                path: route.path.clone(),
                address: route.address.clone(),
                majors: route.majors.clone(),
                credential: credential.to_owned(),
                tokens: route.tokens.clone(),
            })
        })
        .collect()
}

/// `routes`, the media server's presenting `key`.
fn presenting(routes: &[Gated], key: &str) -> Vec<Gated> {
    routes
        .iter()
        .map(|route| match route.target {
            Target::MediaServer { .. } => Gated {
                credential: key.to_owned(),
                ..route.clone()
            },
            Target::Curator { .. } => route.clone(),
        })
        .collect()
}

/// Whether the gate answers presenting `key` on its media server route.
async fn holds(gate: &dyn Gating, key: &str) -> bool {
    let hash = sha256(key);
    gate.admitted().await.is_ok_and(|admitted| {
        admitted.iter().any(|route| {
            matches!(route.target, Target::MediaServer { .. }) && route.credential == hash
        })
    })
}

/// Hand the gate `before` again and revoke `minted`.
async fn put_back(gate: &dyn Gating, client: &dyn AppKeys, before: &[Gated], minted: &str) {
    let _ = gate.admit(before).await;
    let _ = client.revoke(minted).await;
}

#[cfg(test)]
mod tests;
