//! Replacing a credential in the order that leaves a working one at every moment.
//!
//! The ordering is the whole feature. A replacement is set on the live service and
//! proven there *before* lemonfiber's record of the old one is overwritten, so a
//! rotation that fails anywhere leaves the operator with the credential they started
//! with rather than with neither. Every path out of here that is not a landed
//! replacement writes nothing at all.
//!
//! Two credentials can genuinely be replaced from here and the rest cannot, and the
//! difference is not arbitrary. lemonfiber can replace what it minted and set itself
//! — qBittorrent's web UI password — and it can hand out afresh what a service minted
//! for itself, which is the repair for a service that regenerated its key underneath
//! a stack still handing out the old one. It cannot invent a Usenet password: a value
//! this product made up would be one the provider has never heard of, so what is owed
//! there is a sentence saying where a replacement comes from.

use std::path::Path;

use lemonfiber_manifest::{ApiKind, Service};

use crate::app::targets::{record_secret, recorded_secret, service_addr, target_for};
use crate::app::Ctx;
use crate::config;
use crate::credential::{Consumer, Held, Origin, Propagation, Reach, Rotation, Settled, CATALOGUE};
use crate::ports::service::{Client, Failure};
use crate::seed::run::published_as;

/// What a rehearsal says about replacing the credential lemonfiber mints itself.
const MINTING: &str = "a real run would generate a new web UI password, set it on the \
     torrent client, sign in with it to prove the client had taken it, and only then \
     record it. Nothing was generated here, and nothing was set.";

/// What a rehearsal says about handing a service's own key back out.
const REPUBLISHING: &str = "a real run would read the key the service wrote for itself, \
     ask the service to identify itself with it, and — only if it answered — publish \
     that key where the rest of the stack reads it. Nothing was read out and nothing \
     was published.";

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
    project: Option<&Path>,
) -> Rotation {
    if let Some(plugin) = held.plugins() {
        return Rotation::stopped(
            &held.name,
            Settled::Elsewhere {
                detail: held.unheld(plugin),
            },
        );
    }
    match held.origin {
        Origin::Service => republished(ctx, held, services, project).await,
        Origin::Lemonfiber if held.setting == config::QBITTORRENT_PASSWORD_KEY => {
            replaced(ctx, held, services).await
        }
        // A credential whose replacement comes from somewhere else writes nothing on
        // any run, so a rehearsal of it *is* the run: the same sentence, saying where a
        // replacement does come from and that the existing one is untouched.
        Origin::Lemonfiber | Origin::Operator => Rotation::stopped(
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
fn would_rotate(held: &Held, how: &str) -> Rotation {
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
        config::JELLYFIN_ADMIN_PASSWORD_KEY => (
            "Jellyfin's own account settings",
            "lemonfiber config set JELLYFIN_ADMIN_PASSWORD",
        ),
        config::AUDIOBOOKSHELF_PASSWORD_KEY => (
            "Audiobookshelf's own account settings",
            "lemonfiber config set AUDIOBOOKSHELF_PASSWORD",
        ),
        config::BINDERY_API_KEY => (
            "this setting itself — the book service adopts whatever it is given",
            "lemonfiber config set BINDERY_API_KEY",
        ),
        _ => ("wherever this credential was issued", "lemonfiber setup"),
    };
    format!(
        "A replacement for this one comes from {place}, not from here — a value invented here \
         would be one nothing has ever heard of. Change it there, then record it with \
         `{afterwards}`, which proves it against the live service before it keeps it. Until \
         then the existing credential is untouched and still in force."
    )
}

/// Replace qBittorrent's web UI password with a freshly minted one.
///
/// The client sets the replacement and then signs in with it, so a set the service
/// accepted but did not apply is caught rather than called done. Only that confirmed
/// change reaches the record — which is what leaves a failed rotation with the old
/// password still recorded and still the one the service takes.
async fn replaced(ctx: &Ctx, held: &Held, services: &[Service]) -> Rotation {
    // The address and the password it authenticates with are one condition rather
    // than two: a stack with no torrent client has no password recorded for one
    // either, and telling those apart would be telling apart two ways of having
    // nothing to change.
    let Some((addr, current)) =
        service_addr(services, ApiKind::Qbittorrent).zip(recorded_secret(ctx, &held.setting))
    else {
        return unproven(
            held,
            "there is no torrent client with a password recorded, so there is nothing to \
             authenticate with in order to change one; run `lemonfiber seed` to set one",
        );
    };
    // Below everything that can be told without acting and above everything that
    // cannot. A rehearsal has already been through the same two conditions a real run
    // is refused by, so what it reports is what this run would actually attempt — and
    // it stops one line before the replacement is generated, because a password minted
    // to describe a rotation is a secret that exists because somebody asked a question.
    if ctx.dry_run {
        return would_rotate(held, MINTING);
    }
    let Some(replacement) = crate::secret::generate(ctx.seams.random.as_ref()) else {
        return unproven(
            held,
            "no randomness was available to generate a replacement, and a guessable password \
             on the client the forwarded port authenticates to is worse than the one in force",
        );
    };

    let client = crate::qbittorrent::Qbittorrent::new(ctx.seams.http.clone(), &addr.loopback);
    match client.replace_password(&current, &replacement).await {
        Ok(()) => {
            record_secret(ctx, &held.setting, &replacement);
            Rotation::landed(
                &held.name,
                "qBittorrent took the replacement and signed in with it",
                reached(&held.setting),
            )
        }
        Err(Failure::Unauthorised { .. }) => Rotation::stopped(
            &held.name,
            Settled::Refused {
                detail: "qBittorrent refused the password lemonfiber holds, so there was nothing \
                         to change it with. Nothing was written; the recorded password is the \
                         one it was before."
                    .to_owned(),
            },
        ),
        Err(failure) => unproven(held, &said(&failure)),
    }
}

/// Hand a service's own current key back to everything that reads it.
///
/// The repair for a service that regenerated its API key: what it holds now is read
/// from the file it wrote, proven by asking the service to identify itself with it,
/// and only then published where the stack's own services read it. A key that the
/// service will not answer to is not written down, which is the same ordering the
/// minted rotation keeps.
async fn republished(
    ctx: &Ctx,
    held: &Held,
    services: &[Service],
    project: Option<&Path>,
) -> Rotation {
    let Some(target) = services
        .iter()
        .filter_map(|service| project.and_then(|project| target_for(service, project)))
        .find(|target| published_as(&target.id) == held.setting)
    else {
        // Every service that writes its own key is on the inventory, and only the
        // Servarr-shaped ones can be asked to identify themselves with it. For the
        // rest there is nothing here to prove a key against, and handing one out
        // unproven is what seeding already does.
        return Rotation::stopped(
            &held.name,
            Settled::Elsewhere {
                detail: "this service's key is not one lemonfiber can prove by asking the \
                         service who it is, so it is not one this will hand out unproven. Run \
                         `lemonfiber seed`, which reads it and gives it to everything that \
                         reads it from the environment. Until then the existing credential is \
                         untouched and still in force."
                    .to_owned(),
            },
        );
    };
    // Below the one condition that can be told without asking anything — whether this
    // is a service whose key can be proved at all — and above the read of the key
    // itself. Reading it would put a credential in this run's memory to describe a
    // rotation, and asking the service to identify itself with it is an
    // authentication attempt in somebody's log that nobody asked for.
    if ctx.dry_run {
        return would_rotate(held, REPUBLISHING);
    }
    let Some(key) = target.key(ctx.seams.filesystem.as_ref()).await else {
        return unproven(
            held,
            "the service has not written an API key yet, so there is none to hand out",
        );
    };
    let service = crate::servarr::Servarr::new(
        ctx.seams.http.clone(),
        &target.base,
        key.clone(),
        &target.id,
        target.version,
    );
    match service.identity().await {
        Ok(identity) => {
            record_secret(ctx, &held.setting, &key);
            Rotation::landed(
                &held.name,
                &format!(
                    "{} {} answered to the key it holds",
                    identity.name, identity.version
                ),
                super::reading::service_consumers(&target.name, &held.setting)
                    .into_iter()
                    .map(|(consumer, reached)| Propagation {
                        consumer,
                        reach: reached.reach(),
                    })
                    .collect(),
            )
        }
        Err(Failure::Unauthorised { .. }) => Rotation::stopped(
            &held.name,
            Settled::Refused {
                detail: "the service refused the key in its own configuration, so publishing it \
                         would hand every consumer a key that does not work. Nothing was \
                         written. Restart the service so it reloads its configuration, then \
                         try again."
                    .to_owned(),
            },
        ),
        Err(failure) => unproven(held, &said(&failure)),
    }
}

/// A rotation that could not be proven, and so changed nothing.
fn unproven(held: &Held, detail: &str) -> Rotation {
    Rotation::stopped(
        &held.name,
        Settled::Unproven {
            detail: detail.to_owned(),
        },
    )
}

/// What a service said about a failure, with anything credential-shaped in it
/// withheld — the same rule every other sentence a service produces goes through.
fn said(failure: &Failure) -> String {
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
