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

use crate::app::targets::{hosted, record_secret, recorded_secret, service_addr};
use crate::app::Ctx;
use crate::config;
use crate::ports::service::Failure;

/// What a replacement's name ends in, after the name of the credential it replaces.
const PENDING: &str = "_PENDING";

/// A credential lemonfiber mints and replaces itself, and where the service it opens
/// is asked whether it takes one.
struct Minted {
    /// The setting it is kept under.
    setting: String,
    /// Which kind of service it opens.
    kind: ApiKind,
    /// Where the host reaches that service.
    loopback: String,
    /// The service's id.
    id: String,
}

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
    // Only what is asked about is carried across the questions below: the settings file
    // and the stack are read to find it and let go.
    let asked = {
        let Some(env) = ctx.settings.env_file.as_deref() else {
            return;
        };
        let file = config::store::read(env).unwrap_or_default();
        if !file.keys().iter().any(|key| key.ends_with(PENDING)) {
            return;
        }
        // A stack that cannot be read has no service to ask, which leaves both for later.
        let Ok(manifest) = ctx.stack.manifest() else {
            return;
        };
        minted(ctx, &manifest)
    };
    for minted in asked {
        let Some(replacement) = recorded_secret(ctx, &pending(&minted.setting)) else {
            continue;
        };
        let current = recorded_secret(ctx, &minted.setting).unwrap_or_default();
        match taken(ctx, &minted, &current).await {
            Some(true) => forgotten(ctx, &minted.setting),
            Some(false) if taken(ctx, &minted, &replacement).await == Some(true) => {
                let _ = promoted(ctx, &minted.setting, &replacement);
            }
            Some(false) | None => {}
        }
    }
}

/// The credentials lemonfiber mints and replaces itself: the stack's torrent client's
/// web UI password, and the administrator's password of whatever media server fills the
/// identity source, under that server's own setting.
fn minted(ctx: &Ctx, manifest: &lemonfiber_manifest::Manifest) -> Vec<Minted> {
    let torrent = service_addr(&manifest.services, ApiKind::Qbittorrent).map(|addr| Minted {
        setting: config::TORRENT_PASSWORD_KEY.to_owned(),
        kind: ApiKind::Qbittorrent,
        loopback: addr.loopback,
        id: addr.id,
    });
    let media = hosted(ctx, manifest).map(|server| Minted {
        kind: ApiKind::Jellyfin,
        loopback: server.loopback.clone(),
        id: server.id().to_owned(),
        setting: server.setting,
    });
    torrent.into_iter().chain(media).collect()
}

/// Whether the service signs in with `password`: yes, no, or nothing where it would
/// not say.
async fn taken(ctx: &Ctx, minted: &Minted, password: &str) -> Option<bool> {
    let answer = if minted.kind == ApiKind::Qbittorrent {
        crate::qbittorrent::Qbittorrent::new(ctx.seams.http.clone(), &minted.loopback)
            .accepts(password)
            .await
    } else {
        crate::jellyfin::Jellyfin::authenticated(
            ctx.seams.http.clone(),
            &minted.loopback,
            &minted.id,
            config::MEDIA_SERVER_ADMIN_USER,
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
