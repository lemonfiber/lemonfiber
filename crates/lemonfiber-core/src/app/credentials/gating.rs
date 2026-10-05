//! The request gate's own Jellyfin key, as the inventory lists it, prints it and
//! replaces it.
//!
//! It is held in one place, the Jellyfin route of the gate's routes file, which the gate
//! reads on every call. So this line is read from that file, and a replacement is
//! written there.
//!
//! **A replacement keeps a working key at every moment.** A new key is minted, written
//! into the routes and proven against Jellyfin, and only then is the old one revoked.
//! Where any step fails the new key is revoked and the old routes put back. A revocation
//! of the old key that Jellyfin refuses leaves it filed under the gate's name and held by
//! nothing, which the next seed revokes.

use std::path::{Path, PathBuf};

use lemonfiber_manifest::Service;
use lemonfiber_sidecar::gate::{Credential, File, Kind, Upstreams};

use super::declining::{NO_ADMINISTRATOR, UNTAKEN};
use super::rotating::{said, unproven, would_rotate};
use crate::app::gating;
use crate::app::Ctx;
use crate::credential::{fingerprint, Held, Origin, Propagation, Reach, Rotation, State};
use crate::jellyfin::{Jellyfin, GATE_APP};
use crate::seed::run::identity;

/// What the key is recorded as: where it lives inside the stack's configuration.
pub(super) const SETTING: &str = "request-gate/upstreams.json#jellyfin";

/// What the inventory calls it.
const NAME: &str = "Jellyfin request-gate key";

/// The one thing that authenticates with it.
const CONSUMER: &str =
    "the request gate, which reads the media server's libraries and members for the request service";

/// What is said where the key is not there.
const ABSENT: &str = "Jellyfin request-gate key is not there yet, so the request gate cannot \
                      read the media server for the request service. Run `lemonfiber seed`.";

/// What a rehearsal says a replacement would take.
const ROTATING: &str = "a real run would mint a new key on Jellyfin, write it into the request \
                        gate's routes, prove Jellyfin takes it, and only then revoke the old \
                        one. Nothing was minted here, and nothing was written.";

/// What a landed replacement says.
const LANDED: &str = "Jellyfin took the new key, and the request gate's routes hold it";

/// Why a new key was revoked before it was ever used.
const UNWRITTEN: &str =
    "the new key could not be written into the request gate's routes, so it was revoked again";

/// The key's line in the inventory, where the stack runs the gate beside Jellyfin.
pub(super) async fn held(ctx: &Ctx, services: &[Service], project: Option<&Path>) -> Option<Held> {
    gating::service(services)?;
    identity::jellyfin_service(services)?;
    let path = gating::path(project?, File::Upstreams);
    let key = read(ctx, &path).await.and_then(|routes| key_in(&routes));
    Some(Held {
        name: NAME.to_owned(),
        setting: SETTING.to_owned(),
        consumers: vec![CONSUMER.to_owned()],
        location: path.display().to_string(),
        origin: Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state: if key.is_some() {
            State::Active
        } else {
            State::Absent
        },
        fingerprint: key.as_deref().map(fingerprint),
        advisory: key.is_none().then(|| ABSENT.to_owned()),
    })
}

/// The key itself, for the one confirmed ask that prints it.
pub(super) async fn value(ctx: &Ctx, held: &Held) -> Option<String> {
    read(ctx, Path::new(&held.location))
        .await
        .and_then(|routes| key_in(&routes))
}

/// Replace the key, keeping a working one at every moment.
pub(super) async fn rotate(ctx: &Ctx, held: &Held, services: &[Service]) -> Rotation {
    let (Some(jellyfin), Some(password)) = (
        identity::jellyfin_service(services),
        identity::recorded_jellyfin_password(ctx),
    ) else {
        return unproven(held, NO_ADMINISTRATOR);
    };
    if ctx.dry_run {
        return would_rotate(held, ROTATING);
    }
    let client = Jellyfin::authenticated(
        ctx.seams.http.clone(),
        &jellyfin.loopback,
        "jellyfin",
        crate::config::JELLYFIN_ADMIN_USER,
        password,
    );
    let path = PathBuf::from(&held.location);
    // Routes the seed has not written yet have no key to replace, and nothing is minted.
    let Some(old) = read(ctx, &path).await else {
        return unproven(held, ABSENT);
    };
    let minted = match client.mint(GATE_APP).await {
        Ok(minted) => minted,
        Err(failure) => return unproven(held, &said(&failure)),
    };
    let rewritten = (minted.split_whitespace().count() == 1)
        .then(|| holding(&old, &minted))
        .flatten()
        .filter(|routes| crate::config::store::write(&path, &routes.written()).is_ok());
    if rewritten.is_none() {
        let _ = client.revoke(&minted).await;
        return unproven(held, UNWRITTEN);
    }
    if client.answers_to(&minted).await.is_err() {
        let _ = crate::config::store::write(&path, &old.written());
        let _ = client.revoke(&minted).await;
        return unproven(held, UNTAKEN);
    }
    for other in client
        .filed_as(GATE_APP)
        .await
        .unwrap_or_default()
        .iter()
        .filter(|other| **other != minted)
    {
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

/// `routes` with the route to Jellyfin presenting `key`, where they have one.
fn holding(routes: &Upstreams, key: &str) -> Option<Upstreams> {
    let mut upstreams = routes.upstreams.clone();
    let route = upstreams
        .iter_mut()
        .find(|one| one.kind == Kind::Jellyfin)?;
    route.credential = Credential::new(key);
    Some(Upstreams::of(upstreams))
}

/// The key the route to Jellyfin presents, where `routes` have one.
fn key_in(routes: &Upstreams) -> Option<String> {
    routes
        .upstreams
        .iter()
        .find(|one| one.kind == Kind::Jellyfin)
        .map(|route| route.credential.reveal().to_owned())
}

/// The routes the file at `path` holds, where it reads.
async fn read(ctx: &Ctx, path: &Path) -> Option<Upstreams> {
    crate::app::targets::read_owned(
        ctx.seams.filesystem.as_ref(),
        path,
        crate::within::directory_of(path),
    )
    .await
    .and_then(|text| Upstreams::read(&text).ok())
}

#[cfg(test)]
mod tests;
