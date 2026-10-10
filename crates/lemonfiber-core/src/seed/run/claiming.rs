//! Claiming the listening server's first account before anybody else does.
//!
//! The server gives its root account to whoever makes the first one, from anywhere on
//! the network, so an unclaimed one is anybody's. lemonfiber makes it with a password it
//! mints and records first, and a server that already has an account lemonfiber did not
//! make is reported as the takeover it may be rather than passed over.

use lemonfiber_manifest::{ApiKind, Service};

use super::Ctx;
use crate::seed::{State, Wiring};

/// Make the listening server's first account where nobody has, and say how it stands.
///
/// Nothing where the stack has no listening server, and nothing on a run that only says
/// what it would do, which may not make an account. An account already there is
/// lemonfiber's own where it holds the password for it, and somebody else's where it
/// does not — which is the one case worth a failure, since whoever made it holds the
/// server.
pub(super) async fn claimed(ctx: &Ctx, services: &[Service]) -> Option<Wiring> {
    if ctx.dry_run {
        return None;
    }
    let addr = crate::app::targets::service_addr(services, ApiKind::Audiobookshelf)?;
    let name = services
        .iter()
        .find(|service| service.id == addr.id)
        .map_or(addr.id.as_str(), |service| service.name.as_str());
    let client =
        crate::audiobookshelf::Audiobookshelf::new(ctx.seams.http.clone(), addr.loopback, &addr.id);
    let recorded =
        crate::app::targets::recorded_secret(ctx, crate::config::LISTENING_SERVER_PASSWORD_KEY);
    let state = match client.has_account().await {
        Err(failure) => crate::seed::unreached(&failure),
        Ok(true) if recorded.is_some() => State::AlreadyWired,
        Ok(true) => taken(name),
        Ok(false) => made(ctx, &client).await,
    };
    Some(Wiring::settled(format!("{name} first account"), state))
}

/// Mint the first account's password, record it, and only then make the account.
///
/// A record left behind by an account that was not made is taken back off, because a
/// recorded password is what this reads as the account being lemonfiber's own: one
/// left over would read a server somebody else claims later as already claimed.
async fn made(ctx: &Ctx, client: &crate::audiobookshelf::Audiobookshelf) -> State {
    let Some(fresh) = crate::secret::generate(ctx.seams.random.as_ref()) else {
        return State::Failed {
            detail: crate::secret::NO_RANDOMNESS_FOR_PASSWORD.to_owned(),
        };
    };
    if let Err(failure) = crate::app::targets::record_secret(
        ctx,
        crate::config::LISTENING_SERVER_PASSWORD_KEY,
        &fresh,
    ) {
        return State::Failed {
            detail: format!(
                "the password lemonfiber generated could not be recorded, so it was not set: \
                 {failure}"
            ),
        };
    }
    match client
        .create_account(crate::config::LISTENING_SERVER_USER, &fresh)
        .await
    {
        Ok(()) => State::Wired,
        Err(failure) => {
            if let Some(env) = ctx.settings.env_file.as_deref() {
                let _ =
                    crate::config::store::unset(env, crate::config::LISTENING_SERVER_PASSWORD_KEY);
            }
            crate::seed::unreached(&failure)
        }
    }
}

/// A listening server whose first account somebody other than lemonfiber made.
fn taken(name: &str) -> State {
    State::Failed {
        detail: format!(
            "{name} already has an account lemonfiber did not make, and whoever made it first \
             holds the server. If that was not you, someone else on your network claimed it: \
             reset {name}'s configuration and run `lemonfiber seed` to claim it again. If it \
             was you, record its password with `lemonfiber config set {setting} <password>`.",
            setting = crate::config::LISTENING_SERVER_PASSWORD_KEY,
        ),
    }
}
