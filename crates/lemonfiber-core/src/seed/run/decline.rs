//! The decline service's own media server key: minted for it alone, and handed over in a
//! file.
//!
//! The service switches off an invitation the invitee refuses, which on the media server
//! is an administrator's act on every supported line — and no credential narrower than an
//! API key, which administers the whole server, can do it. So the key is the service's
//! and nothing else's: filed under its own name, so the server's key list says what holds
//! it, and written owner-only into the service's configuration directory rather than its
//! environment, which `docker compose config` and `docker inspect` print.
//!
//! **The file is what holds the key.** A key filed under the service's name that the
//! file does not hold is one nothing holds — left by a run interrupted between minting
//! and writing, or by a stack that no longer runs the service — and is revoked, because
//! a credential that administers the server and serves nobody is all risk.

use std::path::Path;

use lemonfiber_sidecar::decline::Key;

use super::Ctx;
use crate::app::invite::declining;
use crate::app::targets::MediaServer;
use crate::app_keys::DECLINE_APP;
use crate::ports::service::{AppKeys, Declining};
use crate::seed::{State, Wiring};

/// What the report calls this connection, for the media server `server`.
fn connection(server: &MediaServer) -> String {
    format!("The decline service's own {} key", server.name())
}

/// Hold the decline service to one key of its own, in the server its link by name
/// reaches — or, where the stack no longer runs the service, to none.
pub(super) async fn seed_decline_key(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&Path>,
) -> Option<Wiring> {
    let server = &crate::app::targets::declined_server(fillers)?;
    let declining = declining::service(services).is_some();
    // Minted with the administrator's session, which lemonfiber holds only on a server
    // it set up. A rehearsal before the first run finds none recorded, because the
    // identity step mints it, and so finds no key on the server either — where the
    // server the decline service acts on is the one the identity step sets up.
    let Some(client) = server.administering(ctx).await else {
        let minting = MediaServer::of(fillers)
            .is_some_and(|identity| identity.id() == server.id() && identity.would_mint(ctx));
        return (declining && minting).then(|| would_mint(server));
    };
    let filed = client.filed_as(DECLINE_APP).await;
    // A stack that does not run the service asked for nothing here: a key list that
    // could not be read is left for the next run to retire from, not reported.
    if !declining {
        return super::minted::retired(
            ctx,
            client.as_ref(),
            DECLINE_APP,
            &connection(server),
            &filed.ok()?,
        )
        .await;
    }
    let filed = match filed {
        Ok(filed) => filed,
        Err(failure) => return Some(settled(server, crate::seed::unreached(&failure))),
    };
    let Some(project) = project else {
        return Some(settled(
            server,
            State::Skipped {
                reason: "there is no stack directory to hand the decline service its key in"
                    .to_owned(),
            },
        ));
    };
    let Some(decline) = declining::asked(ctx, fillers, project).await else {
        return Some(settled(
            server,
            State::Skipped {
                reason: "the decline service cannot be asked over its contract, so it is \
                         handed no key"
                    .to_owned(),
            },
        ));
    };
    let held = decline
        .held()
        .await
        .ok()
        .flatten()
        .and_then(|hash| named_by(&filed, &hash));
    let state = match held {
        Some(key) => kept(ctx, client.as_ref(), &filed, &key).await,
        None if ctx.dry_run => return Some(would_mint(server)),
        None => minted(client.as_ref(), &filed, decline.as_ref()).await,
    };
    Some(settled(server, state))
}

/// The key filed under the service's name whose SHA-256 is `hash`, where one is.
fn named_by(filed: &[String], hash: &str) -> Option<String> {
    filed
        .iter()
        .find(|one| Key::read(one).is_ok_and(|key| key.fingerprint() == hash))
        .cloned()
}

/// The key the service holds is one the media server holds: revoke whatever else is
/// filed beside it.
async fn kept(ctx: &Ctx, client: &dyn AppKeys, filed: &[String], key: &str) -> State {
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

/// Mint a key, hand it to the decline service, and only then revoke what it replaces.
///
/// A key the service would not take is revoked again at once: one nothing holds is not
/// left on the server for the next run to find.
async fn minted(client: &dyn AppKeys, filed: &[String], decline: &dyn Declining) -> State {
    let key = match super::minted::mint(client, DECLINE_APP).await {
        Ok(key) => key,
        Err(state) => return state,
    };
    if let Err(failure) = decline.hold(&key).await {
        let _ = client.revoke(&key).await;
        return State::Failed {
            detail: format!(
                "the decline service would not take the key, so it was revoked again: {failure}"
            ),
        };
    }
    super::minted::revoking(client, filed).await
}

/// What a rehearsal says where a real run would mint the key: nothing of the value,
/// which would not exist yet.
fn would_mint(server: &MediaServer) -> Wiring {
    settled(
        server,
        State::WouldWire {
            yours: None,
            ours: None,
        },
    )
}

/// How many keys are filed under the service's name, said as a reader would.
fn count(keys: usize) -> String {
    super::minted::count(DECLINE_APP, keys)
}

/// This connection to `server`, resting in `state`.
fn settled(server: &MediaServer, state: State) -> Wiring {
    Wiring::settled(connection(server), state)
}
