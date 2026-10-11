//! Replacing the media server's administrator password, over its contracts or through this
//! build's adapter, in the order that leaves the password in force recorded wherever it
//! stops.

use super::super::pending::{forgotten, kept, pending, promoted};
use super::{said, unrecorded, unsigned, Replaced, Replacing};
use crate::app::targets::{record_secret, MediaServer, Reach as Spoken};
use crate::app::Ctx;
use crate::ports::service::Failure;

/// Mint a new administrator password, record it under its pending name, set it on the
/// media server and prove it by signing in with it, and only then move it into place —
/// the order that leaves the recorded password the one in force wherever this stops.
///
/// Kept under the server's own setting throughout, so a plugin's server is given a
/// replacement for its own password and the stack's is never touched for it.
pub(crate) async fn replace_admin_password(
    ctx: &Ctx,
    server: &MediaServer,
    rehearsing: bool,
) -> Result<Replaced, Replacing> {
    match server.reach {
        Spoken::Over => {
            let client = Box::pin(server.setting_up(ctx))
                .await
                .ok_or_else(|| Replacing::Unproven(UNASKED_OVER_ITS_CONTRACTS.to_owned()))?;
            let Some(replacement) = minted(ctx, rehearsing)? else {
                return Ok(Replaced::Rehearsed);
            };
            swapped(
                ctx,
                server,
                &replacement,
                client.replace_admin_password(&replacement),
            )
            .await
        }
        Spoken::Bundled(_) => {
            let client = server
                .signing_in(ctx)
                .map_err(|why| Replacing::Unproven(unsigned(why).to_owned()))?;
            let Some(replacement) = minted(ctx, rehearsing)? else {
                return Ok(Replaced::Rehearsed);
            };
            swapped(
                ctx,
                server,
                &replacement,
                client.replace_password(&replacement),
            )
            .await
        }
    }
}

/// A freshly minted replacement, or nothing where this run only says what it would do.
///
/// Below what can be told without acting: a password generated to describe a rotation is
/// a secret that exists because somebody asked a question.
fn minted(ctx: &Ctx, rehearsing: bool) -> Result<Option<String>, Replacing> {
    if rehearsing {
        return Ok(None);
    }
    crate::secret::generate(ctx.seams.random.as_ref())
        .map(Some)
        .ok_or_else(|| Replacing::Unproven(UNMINTED.to_owned()))
}

/// Record `replacement` as pending, let `change` set it on the server, and move it into
/// place and hand it to the server's adapter only once the server took it.
async fn swapped(
    ctx: &Ctx,
    server: &MediaServer,
    replacement: &str,
    change: impl std::future::Future<Output = Result<(), Failure>>,
) -> Result<Replaced, Replacing> {
    let setting = server.setting.as_str();
    if let Err(failure) = record_secret(ctx, &pending(setting), replacement) {
        return Err(Replacing::Unproven(unrecorded(&failure)));
    }
    match change.await {
        Ok(()) => {
            promoted(ctx, setting, replacement).map_err(Replacing::Unkept)?;
            server
                .hand_upstream_if_contracted(replacement)
                .map(|()| Replaced::Done)
                .map_err(Replacing::Unkept)
        }
        Err(Failure::Unauthorised { .. }) => {
            forgotten(ctx, setting);
            Err(Replacing::Refused)
        }
        Err(failure) => {
            kept(ctx, setting, replacement);
            Err(Replacing::Unproven(said(&failure)))
        }
    }
}

/// What is said where no randomness was available to mint a replacement.
const UNMINTED: &str = "no randomness was available to generate a replacement, and a guessable \
                        password on the account that administers the media server is worse than \
                        the one in force";

/// What is said where the media server speaks its contracts and cannot be asked over them.
pub(super) const UNASKED_OVER_ITS_CONTRACTS: &str =
    "this media server speaks its contracts and cannot be asked over them";
