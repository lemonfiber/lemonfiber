//! The credential a service answers its own API with, as lemonfiber minted it.

use lemonfiber_contract::adapter::{UPSTREAM_PASSWORD, UPSTREAM_USERNAME};

use super::Ctx;
use crate::ports::service::Credential;

/// The credential lemonfiber minted for `filler`, from the secret store, where its plugin
/// declares one and the store holds it.
pub(super) fn minted_key(ctx: &Ctx, filler: &crate::wiring::Filler) -> Option<String> {
    crate::app::targets::stored_secret(ctx, filler.minted.as_ref()?)
}

/// The account the adapter fronting `filler` signs in to it as, from the secret store,
/// where that adapter hands it a username and a password the store holds.
pub(super) fn minted_account(
    ctx: &Ctx,
    fillers: &crate::wiring::Fillers,
    filler: &crate::wiring::Filler,
) -> Option<Credential> {
    let upstream = &fillers.fronting(filler)?.upstream;
    let part = |name: &str| {
        upstream
            .get(name)
            .and_then(|held| crate::app::targets::stored_secret(ctx, held))
    };
    Some(Credential::UserPass {
        username: part(UPSTREAM_USERNAME)?,
        password: part(UPSTREAM_PASSWORD)?,
    })
}

#[cfg(test)]
mod tests;
