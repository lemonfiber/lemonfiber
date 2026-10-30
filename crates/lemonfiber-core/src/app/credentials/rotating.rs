//! Replacing a credential in the order that leaves a working one at every moment.
//!
//! The ordering is the whole feature. A replacement lemonfiber mints is recorded under a
//! pending name *before* the service is given it, and moved over the credential it
//! replaces only once the service has taken it and signed in with it (see
//! [`super::pending`]). A key a service minted for itself is proven against the service
//! first and published only once it answers. Either way a rotation that fails anywhere
//! leaves the operator with the credential they started with rather than with neither,
//! and never with a password set on a service that nothing recorded.
//!
//! What can genuinely be replaced from here is what lemonfiber minted and set itself — the
//! media server's administrator password and every secret a plugin declares minted — and
//! the rest cannot, and the difference is not arbitrary. It cannot invent a Usenet
//! password: a value this product made up would be one the provider has never heard of,
//! so what is owed there is a sentence saying where a replacement comes from.

use std::path::Path;

use lemonfiber_manifest::Service;

use crate::app::targets::{recorded_secret, MediaServer};
use crate::app::Ctx;
use crate::config;
use crate::credential::{Consumer, Held, Origin, Propagation, Reach, Rotation, Settled, CATALOGUE};
use crate::ports::service::Failure;

mod replacing;
pub(crate) use replacing::replace_admin_password;

/// What a rehearsal says about replacing the media server's administrator password.
const ADMINISTERING: &str = "a real run would generate a new administrator password, record it \
     beside the one in force, set it on the media server, sign in with it to prove the server \
     took it, and only then put it in place of the old one. Nothing was generated here, and \
     nothing was set.";

/// Replace the credential one inventory line names — or, on a run that only says what
/// it would do, say what replacing it would take.
///
/// The two paths that write hold that gate themselves rather than having it held here,
/// and each holds it below everything about the machine that can be told without
/// acting: a rehearsal is turned back wherever a real run would be turned back, and
/// reports only where a real run would actually attempt something. The third path
/// writes nothing on any run, so a rehearsal of it is the run.
pub(crate) async fn rotate(
    ctx: &Ctx,
    held: &Held,
    services: &[Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&Path>,
) -> Rotation {
    if let Some(plugin) = held.plugins() {
        let register = crate::app::plugins::read(ctx).unwrap_or_default();
        if let Some((installed, secret)) =
            super::minting::minted(held, plugin, register.installed())
        {
            return super::minting::rotate(
                ctx,
                held,
                installed,
                secret,
                &super::minting::Amid {
                    services,
                    fillers,
                    project,
                },
            )
            .await;
        }
        return Rotation::stopped(
            &held.name,
            Settled::Elsewhere {
                detail: held.unheld(plugin),
            },
        );
    }
    match held.origin {
        Origin::Lemonfiber if held.setting == super::declining::SETTING => {
            super::declining::rotate(ctx, held, fillers, project).await
        }
        Origin::Lemonfiber if held.setting == super::gating::SETTING => {
            super::gating::rotate(ctx, held, fillers, project).await
        }
        Origin::Lemonfiber if super::tokening::is_token(held) => {
            super::tokening::rotate(ctx, held, services, fillers, project).await
        }
        Origin::Lemonfiber if held.setting == config::IDENTITY_SOURCE_ADMIN_PASSWORD_KEY => {
            administrator(ctx, held, fillers).await
        }
        // A credential whose replacement comes from somewhere else writes nothing on
        // any run, so a rehearsal of it *is* the run: the same sentence, saying where a
        // replacement does come from and that the existing one is untouched.
        Origin::Lemonfiber | Origin::Operator | Origin::Service => Rotation::stopped(
            &held.name,
            Settled::Elsewhere {
                detail: elsewhere(&held.setting),
            },
        ),
    }
}

/// What a rotation would replace, where the value lives, and what would be owed after.
///
/// Nothing is generated and nothing is asked of the service. The consumers that still
/// need something done are read off the same catalogue a landed rotation reports them
/// from, so the list an operator plans around is the list they will be given — and the
/// one consumer a rotation reaches by itself is left out, because it is not a step.
pub(super) fn would_rotate(held: &Held, how: &str) -> Rotation {
    Rotation::would(
        &held.name,
        how,
        &held.location,
        reached(&held.setting)
            .into_iter()
            .filter_map(|one| match one.reach {
                Reach::Pending { detail } => Some(format!("{} — {detail}", one.consumer)),
                Reach::Updated | Reach::Failed { .. } => None,
            })
            .collect(),
    )
}

/// Where a replacement for a credential this cannot rotate actually comes from.
///
/// One sentence per credential rather than one for all of them, because "change it
/// somewhere else" is not an instruction. What the operator needs is the name of the
/// place and the command that takes the new value afterwards.
fn elsewhere(setting: &str) -> String {
    let (place, afterwards) = match setting {
        crate::credential::VPN_PRIVATE_KEY => (
            "your VPN provider's own account page, where a new key is generated",
            "lemonfiber config set WIREGUARD_PRIVATE_KEY",
        ),
        config::INDEXER_APIKEY_KEY => (
            "your indexer's own account page, which is where the key was issued",
            "lemonfiber setup",
        ),
        config::PROVIDER_PASS_KEY => (
            "your Usenet provider's own account page",
            "lemonfiber setup",
        ),
        _ => ("wherever this credential was issued", "lemonfiber setup"),
    };
    format!(
        "A replacement for this one comes from {place}, not from here — a value invented here \
         would be one nothing has ever heard of. Change it there, then record it with \
         {afterwards}, which proves it against the live service before it keeps it. Until \
         then the existing credential is untouched and still in force."
    )
}

/// Why a rotation that could not record its replacement set nothing.
fn unrecorded(failure: &config::store::Failure) -> String {
    crate::config::store::withheld_text(&format!(
        "the replacement could not be recorded: {failure}. It was not set on the service, \
         because a password lemonfiber has no record of is one it would be locked out by; \
         the existing credential is the one still in force."
    ))
}

/// Replace the media server's administrator password with a freshly minted one.
async fn administrator(ctx: &Ctx, held: &Held, fillers: &crate::wiring::Fillers) -> Rotation {
    // The media server whose administrator's password this is: the one filling the
    // identity source, where its password is kept under this credential's setting and
    // recorded there. Boxed, because it is carried across the replacement below.
    let Some(server) = MediaServer::of(fillers)
        .filter(|server| server.setting == held.setting)
        .filter(|server| recorded_secret(ctx, &server.setting).is_some())
        .map(Box::new)
    else {
        return unproven(held, NO_ADMINISTRATOR_HELD);
    };
    match replace_admin_password(ctx, &server, ctx.dry_run).await {
        Ok(Replaced::Rehearsed) => would_rotate(held, ADMINISTERING),
        Ok(Replaced::Done) => Rotation::landed(
            &held.name,
            &format!(
                "{} took the new password and signed in with it",
                server.name()
            ),
            reached(&held.setting),
        ),
        Err(Replacing::Refused) => Rotation::stopped(
            &held.name,
            Settled::Refused {
                detail: format!(
                    "{} refused the password lemonfiber holds, so there was nothing to change \
                     it with. Nothing was written; the recorded password is the one it was \
                     before.",
                    server.name()
                ),
            },
        ),
        Err(Replacing::Unproven(detail)) => unproven(held, &detail),
        Err(Replacing::Unkept(detail)) => {
            Rotation::stopped(&held.name, Settled::ReplacedUnproven { detail })
        }
    }
}

/// How far a replacement of the media server's administrator password went.
pub(crate) enum Replaced {
    /// Set, proven and recorded.
    Done,
    /// This run only says what it would do, and a real run would get as far as minting.
    Rehearsed,
}

/// Why the media server's administrator password was not replaced.
pub(crate) enum Replacing {
    /// The media server refused the password lemonfiber holds.
    Refused,
    /// Nothing usable answered, or there was nothing to replace with; why, in words.
    Unproven(String),
    /// The media server took the replacement and it could not be moved into place; why, in
    /// words.
    Unkept(String),
}

/// What is said where lemonfiber holds no administrator password for the media server.
const NO_ADMINISTRATOR_HELD: &str = "lemonfiber holds no administrator password for this media \
                                     server, so there is nothing to change; run lemonfiber seed";

/// A rotation that could not be proven, and so changed nothing.
pub(super) fn unproven(held: &Held, detail: &str) -> Rotation {
    Rotation::stopped(
        &held.name,
        Settled::Unproven {
            detail: detail.to_owned(),
        },
    )
}

/// What a service said about a failure, with anything credential-shaped in it
/// withheld — the same rule every other sentence a service produces goes through.
pub(super) fn said(failure: &Failure) -> String {
    crate::config::store::withheld_text(&format!(
        "the replacement could not be proven: {failure}. Nothing was written; the existing \
         credential is the one still in force."
    ))
}

/// How far a landed replacement has reached each consumer of this credential.
///
/// Read off the declared consumer list rather than written out again here, so a
/// consumer added to a credential turns up in the next rotation's report with nobody
/// having had to remember this file exists.
fn reached(setting: &str) -> Vec<Propagation> {
    CATALOGUE
        .iter()
        .find(|entry| entry.setting == setting)
        .map(|entry| entry.consumers.iter().map(Consumer::reached).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
