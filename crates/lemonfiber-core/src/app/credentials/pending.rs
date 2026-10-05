//! A minted replacement, kept beside the credential it replaces until the service has
//! taken it.
//!
//! The record is the only copy lemonfiber has of a password it minted, so a service
//! given one that was never written down is a service lemonfiber is locked out of. And
//! the credential in force is not overwritten until its replacement is proven, so a
//! rotation that fails anywhere leaves the old one where it was. Both hold because the
//! replacement is written under a name of its own first, set on the service, proven,
//! and only then moved into place.
//!
//! A run that stops between the set and the move leaves the two side by side, and
//! which one the service takes is then a question for the service: [`settled`] asks it
//! at the start of the next command, before anything signs in with either.

use lemonfiber_manifest::ApiKind;

use crate::app::targets::{record_secret, recorded_secret, service_addr};
use crate::app::Ctx;
use crate::config;
use crate::ports::service::Failure;

/// What a replacement's name ends in, after the name of the credential it replaces.
const PENDING: &str = "_PENDING";

/// The credentials lemonfiber mints and replaces itself, and the service each opens.
const MINTED: [(&str, ApiKind); 2] = [
    (config::QBITTORRENT_PASSWORD_KEY, ApiKind::Qbittorrent),
    (config::JELLYFIN_ADMIN_PASSWORD_KEY, ApiKind::Jellyfin),
];

/// The name a replacement for `setting` is kept under until the service has taken it.
pub(crate) fn pending(setting: &str) -> String {
    format!("{setting}{PENDING}")
}

/// Move a replacement the service has taken into place, and take its pending copy away.
///
/// # Errors
///
/// What the operator is owed where the move would not write: the replacement is in
/// force and is still kept under its pending name, which the next command settles.
pub(super) fn promoted(ctx: &Ctx, setting: &str, replacement: &str) -> Result<(), String> {
    record_secret(ctx, setting, replacement).map_err(|failure| {
        config::store::withheld_text(&format!(
            "the service took the replacement, but it could not be moved into place in the \
             record: {failure}. It is kept under {}, and the next command signs in with it \
             and moves it.",
            pending(setting)
        ))
    })?;
    forgotten(ctx, setting);
    Ok(())
}

/// Write a replacement's pending copy again, from the value the rotation still holds,
/// once the service has left it unknown whether the replacement was taken.
///
/// Another command may have settled the pending copy while the service was being asked,
/// having found the credential in force still taken a moment before the service was
/// given its replacement. The copy written here is what the next command asks about, so
/// a replacement the service did take is never left without a record. Best effort: a
/// file that would not take it a moment ago will not take it now either, and what is
/// owed the operator is already said by the failure that brought the rotation here.
pub(super) fn kept(ctx: &Ctx, setting: &str, replacement: &str) {
    let _ = record_secret(ctx, &pending(setting), replacement);
}

/// Take a replacement's pending copy away, where there is a file to take it from.
///
/// Best effort: a copy left behind is only ever read where the credential beside it
/// is refused, and [`settled`] takes it away the next time it is asked.
pub(super) fn forgotten(ctx: &Ctx, setting: &str) {
    if let Some(env) = ctx.settings.env_file.as_deref() {
        let _ = config::store::unset(env, &pending(setting));
    }
}

/// Settle every replacement a run left pending: whichever of the two the service takes
/// stays, under the credential's own name, and the pending copy goes.
///
/// Nothing is asked of any service where nothing is pending, which is every run but the
/// one after a rotation that stopped part-way. Where the service takes neither, or will
/// not answer, both are left for a later run to ask again. A run that only says what it
/// would do writes nothing, so it settles nothing either.
pub(crate) async fn settled(ctx: &Ctx) {
    if ctx.dry_run {
        return;
    }
    for (setting, kind) in MINTED {
        let Some(replacement) = recorded_secret(ctx, &pending(setting)) else {
            continue;
        };
        let Ok(manifest) = ctx.stack.manifest() else {
            return;
        };
        let Some(addr) = service_addr(&manifest.services, kind) else {
            continue;
        };
        let current = recorded_secret(ctx, setting).unwrap_or_default();
        match taken(ctx, kind, &addr, &current).await {
            Some(true) => forgotten(ctx, setting),
            Some(false) if taken(ctx, kind, &addr, &replacement).await == Some(true) => {
                let _ = promoted(ctx, setting, &replacement);
            }
            Some(false) | None => {}
        }
    }
}

/// Whether the service signs in with `password`: yes, no, or nothing where it would
/// not say.
async fn taken(
    ctx: &Ctx,
    kind: ApiKind,
    addr: &crate::app::targets::ServiceAddr,
    password: &str,
) -> Option<bool> {
    let answer = if kind == ApiKind::Qbittorrent {
        crate::qbittorrent::Qbittorrent::new(ctx.seams.http.clone(), &addr.loopback)
            .accepts(password)
            .await
    } else {
        crate::jellyfin::Jellyfin::authenticated(
            ctx.seams.http.clone(),
            &addr.loopback,
            &addr.id,
            config::JELLYFIN_ADMIN_USER,
            password,
        )
        .accepts()
        .await
    };
    match answer {
        Ok(()) => Some(true),
        Err(Failure::Unauthorised { .. }) => Some(false),
        Err(_) => None,
    }
}
