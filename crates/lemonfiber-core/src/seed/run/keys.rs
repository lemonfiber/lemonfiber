//! The key a service answers its own API to, and what a connection comes to where the
//! file holding it was refused.

use lemonfiber_contract::adapter::{UPSTREAM_PASSWORD, UPSTREAM_USERNAME};

use super::Ctx;
use crate::ports::filesystem::Beneath;
use crate::ports::service::Credential;

/// The API key a service answers to: the one lemonfiber minted for it where its plugin
/// declares one, and otherwise the one it wrote for itself, read from the file its own
/// declaration names by the shape that declaration gives it, beneath its own directory.
///
/// [`Beneath::Read`] holds the key itself; a file holding none, or of a shape no key is
/// read from, is as absent as one not written, and a file refused stays refused, so the
/// caller can say so.
pub(super) async fn curator_key(ctx: &Ctx, filler: &crate::wiring::Filler) -> Beneath {
    if filler.minted.is_some() {
        return minted_key(ctx, filler).map_or(Beneath::Absent, Beneath::Read);
    }
    let Some(read) = filler
        .adapter
        .as_ref()
        .and_then(|api| crate::credential::written::reader(api.key_source))
    else {
        return Beneath::Absent;
    };
    match crate::app::targets::credential_file(ctx, filler).await {
        Beneath::Read(text) => read(&text).map_or(Beneath::Absent, Beneath::Read),
        other => other,
    }
}

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

/// A connection refused because the filler's credential file was, saying why.
///
/// Refused rather than skipped: no later run reads the file while it stays what it is,
/// and it is either a mistake in the plugin or an attempt by it, which the operator has
/// to see either way.
pub(super) fn refused(connection: String, filler: &crate::wiring::Filler) -> crate::seed::Wiring {
    crate::seed::Wiring::settled(connection, refusal(filler))
}

/// What a connection comes to where the filler's credential file was refused.
pub(super) fn refusal(filler: &crate::wiring::Filler) -> crate::seed::State {
    crate::seed::State::Refused {
        reason: crate::app::targets::escaped(filler),
    }
}

#[cfg(test)]
mod tests;
