//! The settings a service's key is published under, and the one an older stack published
//! for a dashboard, retired.
//!
//! A first-party part answers to the key lemonfiber minted for it, and every secrets file
//! naming that key reads it from the secret store each time it is written, so no copy of
//! it is kept in the environment file. The dashboard is published to the household
//! network and holds no credential of any service, so a media server key filed under
//! lemonfiber's name is revoked and its setting forgotten.

use super::Ctx;
use crate::wiring::Fillers;

/// The environment name a service's key is published under.
pub(crate) fn published_as(id: &str) -> String {
    crate::config::for_service(id, crate::config::API_KEY_SUFFIX)
}

/// Revoke the media server key filed under lemonfiber's name, and forget the media
/// server's setting in the environment file.
///
/// Forgotten only once its key is off the server — or was never there — so a revocation
/// that failed leaves the value where the next run can still find which key it was. A
/// rehearsal retires nothing.
pub(super) async fn retire_keys(ctx: &Ctx, fillers: &Fillers) {
    let (false, Some(env)) = (ctx.dry_run, ctx.settings.env_file.as_deref()) else {
        return;
    };
    let server = crate::app::targets::MediaServer::of(fillers);
    let client = match &server {
        Some(server) => server.administering(ctx).await,
        None => None,
    };
    let revoked = match client {
        Some(client) => crate::app_keys::revoke_ours(client.as_ref()).await.ok(),
        None => None,
    };
    // Only what is there: the file is rewritten by every removal, and a setting that
    // was never published is not a reason to touch it.
    if let Some(setting) = server
        .filter(|_| revoked.is_some())
        .map(|server| published_as(server.id()))
        .filter(|setting| crate::app::targets::recorded_secret(ctx, setting).is_some())
    {
        let _ = crate::config::store::unset(env, &setting);
    }
}

#[cfg(test)]
mod tests;
