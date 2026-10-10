//! The decline service's media server key, as the inventory lists it, prints it and
//! replaces it.
//!
//! It is held in one place, the key file in the service's configuration directory,
//! and nowhere in the settings: the environment is printed by `docker compose config`
//! and `docker inspect`. So this line is read from that file, and a replacement is
//! written there.
//!
//! **A replacement keeps a working key at every moment.** A new key is minted, written,
//! proven against the media server and confirmed by the service, and only then is the
//! old one revoked. Where any step fails the new key is revoked and the old one put back.
//! A revocation of the old key that the media server refuses leaves it filed under the
//! service's name and held by nothing, which the next seed revokes.

use std::path::{Path, PathBuf};

use lemonfiber_manifest::Service;
use lemonfiber_sidecar::decline::{File, Key};

use super::rotating::{said, unproven, would_rotate};
use crate::app::invite::declining;
use crate::app::Ctx;
use crate::app_keys::DECLINE_APP;
use crate::credential::{fingerprint, Held, Origin, Propagation, Reach, Rotation, State};
use crate::jellyfin::Jellyfin;
use crate::ports::service::AppKeys as _;

/// What the key is recorded as: where it lives inside the stack's configuration.
pub(super) const SETTING: &str = "decline/jellyfin.key";

/// What the inventory calls it.
const NAME: &str = "Media server decline key";

/// The one thing that authenticates with it.
const CONSUMER: &str = "the decline service, which switches off an invitation the invitee refuses";

/// What is said where the key is not there.
const ABSENT: &str = "Media server decline key is not there yet, so the decline service cannot \
                      switch off an invitation anybody refuses. Run `lemonfiber seed`.";

/// What a rehearsal says a replacement would take.
const ROTATING: &str = "a real run would mint a new key on the media server, write it where \
                        the decline service reads it, prove the media server takes it and \
                        that the service holds it, and only then revoke the old one. Nothing \
                        was minted here, and nothing was written.";

/// What a landed replacement says.
const LANDED: &str = "The media server took the new key, and the decline service holds it";

/// Why there is nothing to mint with.
pub(super) const NO_ADMINISTRATOR: &str =
    "lemonfiber holds no administrator for this media server, so there is nothing to mint \
     a key with; run `lemonfiber seed`";

/// Why a new key was revoked before it was ever used.
const UNWRITTEN: &str =
    "the new key could not be written where the decline service reads it, so it was revoked again";

/// Why a new key was revoked once the media server refused it.
pub(super) const UNTAKEN: &str =
    "The media server did not take the new key, so it was revoked and the old one put back";

/// Why a new key was revoked once the service did not hold it.
const UNHELD: &str = "the decline service did not report holding the new key, so it was revoked \
                      and the old one put back";

/// The key's line in the inventory, where the stack runs the decline service.
pub(super) async fn held(ctx: &Ctx, services: &[Service], project: Option<&Path>) -> Option<Held> {
    declining::service(services)?;
    let path = declining::path(project?, File::Key);
    let key = read(ctx, &path).await;
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
        fingerprint: key.as_ref().map(|key| fingerprint(key.reveal())),
        advisory: key.is_none().then(|| ABSENT.to_owned()),
    })
}

/// The key itself, for the one confirmed ask that prints it.
pub(super) async fn value(ctx: &Ctx, held: &Held) -> Option<String> {
    read(ctx, Path::new(&held.location))
        .await
        .map(|key| key.reveal().to_owned())
}

/// Replace the key, keeping a working one at every moment.
pub(super) async fn rotate(
    ctx: &Ctx,
    held: &Held,
    services: &[Service],
    fillers: &crate::wiring::Fillers,
) -> Rotation {
    let Some(client) =
        crate::app::targets::declined_server(fillers).and_then(|server| server.administered(ctx))
    else {
        return unproven(held, NO_ADMINISTRATOR);
    };
    let health = declining::service(services)
        .and_then(|service| service.port)
        .map(|port| {
            crate::decline::Decline::new(ctx.seams.http.clone(), format!("http://127.0.0.1:{port}"))
        });
    if ctx.dry_run {
        return would_rotate(held, ROTATING);
    }
    let path = PathBuf::from(&held.location);
    let old = read(ctx, &path).await;
    let minted = match client.mint(DECLINE_APP).await {
        Ok(minted) => minted,
        Err(failure) => return unproven(held, &said(&failure)),
    };
    let written = Key::read(&minted)
        .ok()
        .filter(|key| crate::config::store::write(&path, &key.written()).is_ok());
    let Some(key) = written else {
        let _ = client.revoke(&minted).await;
        return unproven(held, UNWRITTEN);
    };
    if client.answers_to(&minted).await.is_err() {
        put_back(ctx, &client, &path, old.as_ref(), &minted).await;
        return unproven(held, UNTAKEN);
    }
    let holds = match health {
        Some(service) => service.health().await.is_ok_and(|said| said.holds(&key)),
        None => false,
    };
    if !holds {
        put_back(ctx, &client, &path, old.as_ref(), &minted).await;
        return unproven(held, UNHELD);
    }
    for other in client
        .filed_as(DECLINE_APP)
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

/// Revoke `minted` and put `old` back where the service reads it, or leave nothing there
/// where there was nothing before.
async fn put_back(ctx: &Ctx, client: &Jellyfin, path: &Path, old: Option<&Key>, minted: &str) {
    match old {
        Some(old) => {
            let _ = crate::config::store::write(path, &old.written());
        }
        None => ctx.seams.filesystem.remove(path).await,
    }
    let _ = client.revoke(minted).await;
}

/// The key the file at `path` holds, where it holds one.
async fn read(ctx: &Ctx, path: &Path) -> Option<Key> {
    crate::app::targets::read_owned(
        ctx.seams.filesystem.as_ref(),
        path,
        crate::within::directory_of(path),
    )
    .await
    .and_then(|text| Key::read(&text).ok())
}

#[cfg(test)]
mod tests;
