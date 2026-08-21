//! The secrets lemonfiber minted rather than read.
//!
//! Almost every credential in the stack is a service's own, read from where it wrote it.
//! The exceptions are recorded beside the environment file, because there is nowhere else
//! they could be read back from.

use crate::app::Ctx;
use crate::config::store;

/// A secret lemonfiber minted and recorded in the environment file, read back by
/// its key, or nothing where none is recorded yet — an unreadable or absent file
/// reads the same as an empty value. The one reader for every credential lemonfiber
/// mints and records rather than reads from a service (qBittorrent's password,
/// Jellyfin's admin password), so the read-back stays identical across them.
pub(crate) fn recorded_secret(ctx: &Ctx, key: &str) -> Option<String> {
    let path = ctx.settings.env_file.as_deref()?;
    let file = store::read(path).unwrap_or_default();
    let value = file.get(key)?;
    (!value.is_empty()).then(|| value.to_owned())
}

/// The write side of [`recorded_secret`]: record a credential lemonfiber minted
/// where a later run — and the dashboard — reads it back, or nowhere when there is
/// no environment file to keep it in. Best-effort, like the other records seeding
/// keeps: a run that cannot persist it still set the secret on the service.
pub(crate) fn record_secret(ctx: &Ctx, key: &str, value: &str) {
    if let Some(path) = ctx.settings.env_file.as_deref() {
        let _ = store::set(path, key, value);
    }
}

/// The qBittorrent web UI password recorded at seeding — read back for the
/// dashboard's transfers authentication and for a later seed run.
pub(crate) fn recorded_qbittorrent_password(ctx: &Ctx) -> Option<String> {
    recorded_secret(ctx, crate::config::QBITTORRENT_PASSWORD_KEY)
}
