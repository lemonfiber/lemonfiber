//! The decline service's own Jellyfin key: minted for it alone, and handed over in a file.
//!
//! The service switches off an invitation the invitee refuses, which on Jellyfin is an
//! administrator's act on every supported line — and no credential narrower than an API
//! key, which administers the whole server, can do it. So the key is the service's and
//! nothing else's: filed under its own name, so Jellyfin's key list says what holds it,
//! and written owner-only into the service's configuration directory rather than its
//! environment, which `docker compose config` and `docker inspect` print.
//!
//! **The file is what holds the key.** A key filed under the service's name that the
//! file does not hold is one nothing holds — left by a run interrupted between minting
//! and writing, or by a stack that no longer runs the service — and is revoked, because
//! a credential that administers the server and serves nobody is all risk.

use std::path::Path;

use lemonfiber_sidecar::decline::{File, Key};

use super::Ctx;
use crate::app::invite::declining;
use crate::app::targets::MediaServer;
use crate::app_keys::DECLINE_APP;
use crate::jellyfin::Jellyfin;
use crate::ports::service::AppKeys as _;
use crate::seed::{State, Wiring};

/// What the report calls this connection.
const CONNECTION: &str = "The decline service's own Jellyfin key";

/// Hold the decline service to one key of its own, where the stack has Jellyfin — or,
/// where the stack no longer runs the service, to none.
pub(super) async fn seed_decline_key(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    server: Option<&MediaServer>,
    project: Option<&Path>,
) -> Option<Wiring> {
    let jellyfin = super::identity::jellyfin_service(services)?;
    let declining = declining::service(services).is_some();
    // Minted with the administrator's session, which lemonfiber holds only on a server
    // it set up. A rehearsal before the first run finds none recorded, because the
    // identity step mints it, and so finds no key on the server either.
    let Some(password) = super::identity::recorded_jellyfin_password(ctx) else {
        let minting = server.is_some_and(|server| {
            server.setting == crate::config::JELLYFIN_ADMIN_PASSWORD_KEY && server.would_mint(ctx)
        });
        return (declining && minting).then(would_mint);
    };
    let client = Jellyfin::authenticated(
        ctx.seams.http.clone(),
        &jellyfin.loopback,
        "jellyfin",
        crate::config::JELLYFIN_ADMIN_USER,
        password,
    );
    let filed = client.filed_as(DECLINE_APP).await;
    // A stack that does not run the service asked for nothing here: a key list that
    // could not be read is left for the next run to retire from, not reported.
    if !declining {
        return super::minted::retired(ctx, &client, DECLINE_APP, CONNECTION, &filed.ok()?).await;
    }
    let filed = match filed {
        Ok(filed) => filed,
        Err(failure) => return Some(settled(crate::seed::unreached(&failure))),
    };
    let Some(project) = project else {
        return Some(settled(State::Skipped {
            reason: "there is no stack directory to hand the decline service its key in".to_owned(),
        }));
    };
    let path = declining::path(project, File::Key);
    let held = crate::app::targets::read_owned(
        ctx.seams.filesystem.as_ref(),
        &path,
        crate::within::directory_of(&path),
    )
    .await
    .and_then(|text| Key::read(&text).ok())
    .filter(|key| filed.iter().any(|one| one == key.reveal()));
    let state = match held {
        Some(key) => kept(ctx, &client, &filed, key.reveal()).await,
        None if ctx.dry_run => return Some(would_mint()),
        None => minted(&client, &filed, &path).await,
    };
    Some(settled(state))
}

/// The key the file holds is one Jellyfin holds: revoke whatever else is filed beside it.
async fn kept(ctx: &Ctx, client: &Jellyfin, filed: &[String], key: &str) -> State {
    let others: Vec<&String> = filed.iter().filter(|one| *one != key).collect();
    if others.is_empty() {
        return State::AlreadyWired;
    }
    if ctx.dry_run {
        return State::WouldWire {
            yours: Some(count(others.len())),
            ours: Some(count(1)),
        };
    }
    super::minted::revoking(client, others).await
}

/// Mint a key, write it where the service reads it, and only then revoke what it replaces.
///
/// A key that could not be written is revoked again at once: one nothing holds is not
/// left on the server for the next run to find.
async fn minted(client: &Jellyfin, filed: &[String], path: &Path) -> State {
    let key = match super::minted::mint(client, DECLINE_APP).await {
        Ok(key) => key,
        Err(state) => return state,
    };
    // One word, which is what minting it settled, so it reads as a key.
    let written = Key::read(&key).map(|key| key.written()).unwrap_or_default();
    if let Err(failure) = crate::config::store::write(path, &written) {
        let _ = client.revoke(&key).await;
        return State::Failed {
            detail: format!(
                "the key could not be written to {}, so it was revoked again: {failure}",
                path.display()
            ),
        };
    }
    super::minted::revoking(client, filed).await
}

/// What a rehearsal says where a real run would mint the key: nothing of the value,
/// which would not exist yet.
fn would_mint() -> Wiring {
    settled(State::WouldWire {
        yours: None,
        ours: None,
    })
}

/// How many keys are filed under the service's name, said as a reader would.
fn count(keys: usize) -> String {
    super::minted::count(DECLINE_APP, keys)
}

/// This connection, resting in `state`.
fn settled(state: State) -> Wiring {
    Wiring::settled(CONNECTION.to_owned(), state)
}
