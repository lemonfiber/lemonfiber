//! Replacing the media server's administrator password, over its contracts or through this
//! build's adapter, in the order that leaves the password in force recorded wherever it
//! stops.

use std::sync::Arc;

use lemonfiber_contract::capabilities::identity::source;

use super::super::pending::{forgotten, kept, pending, promoted};
use super::{said, unrecorded, Replaced, Replacing};
use crate::app::targets::{record_secret, MediaServer};
use crate::app::Ctx;
use crate::ports::service::Failure;

/// Mint a new administrator password, record it under its pending name, set it on the
/// media server and prove it by signing in with it, and only then move it into place and
/// hand it to the server's adapter — the order that leaves the recorded password the one
/// in force wherever this stops.
///
/// Kept under the server's own setting throughout, so a plugin's server is given a
/// replacement for its own password and the stack's is never touched for it. A rehearsal
/// stops before the mint, so no password exists because somebody asked a question.
pub(crate) async fn replace_admin_password(
    ctx: &Ctx,
    server: &MediaServer,
    rehearsing: bool,
) -> Result<Replaced, Replacing> {
    let client = asked(ctx, server).await?;
    if rehearsing {
        return Ok(Replaced::Rehearsed);
    }
    let replacement = crate::secret::generate(ctx.seams.random.as_ref())
        .ok_or_else(|| Replacing::Unproven(UNMINTED.to_owned()))?;
    let setting = server.setting.as_str();
    if let Err(failure) = record_secret(
        ctx,
        &pending(setting),
        &replacement,
        crate::app::credentials::OPERATION,
    ) {
        return Err(Replacing::Unproven(unrecorded(&failure)));
    }
    match client.replace_admin_password(&replacement).await {
        Ok(()) => promoted(ctx, setting, &replacement)
            .and_then(|()| server.hand_upstream(&replacement))
            .map(|()| Replaced::Done)
            .map_err(Replacing::Unkept),
        Err(Failure::Unauthorised { .. }) => {
            forgotten(ctx, setting);
            Err(Replacing::Refused)
        }
        Err(failure) => {
            kept(ctx, setting, &replacement);
            Err(Replacing::Unproven(said(&failure)))
        }
    }
}

/// The media server as its administrator's password is changed on it, over
/// `identity.source`.
async fn asked(ctx: &Ctx, server: &MediaServer) -> Result<Arc<dyn source::Fills>, Replacing> {
    Box::pin(server.identifying(ctx))
        .await
        .ok_or_else(|| Replacing::Unproven(UNASKED_OVER_ITS_CONTRACTS.to_owned()))
}

/// What is said where no randomness was available to mint a replacement.
const UNMINTED: &str = "no randomness was available to generate a replacement, and a guessable \
                        password on the account that administers the media server is worse than \
                        the one in force";

/// What is said where the media server speaks its contracts and cannot be asked over them.
pub(super) const UNASKED_OVER_ITS_CONTRACTS: &str =
    "this media server speaks its contracts and cannot be asked over them";
