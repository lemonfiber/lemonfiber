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
/// mints and records rather than reads from a service (the torrent client's password,
/// the media server's admin password), so the read-back stays identical across them.
pub(crate) fn recorded_secret(ctx: &Ctx, key: &str) -> Option<String> {
    let path = ctx.settings.env_file.as_deref()?;
    let file = store::read(path).unwrap_or_default();
    let value = file.get(key)?;
    (!value.is_empty()).then(|| value.to_owned())
}

/// Which service the operator chose to fill each capability, and what they said about
/// each choice, as the environment file records them.
///
/// Read through the same seam every other recorded answer is, and answering with
/// nothing chosen where there is no file: a stack that has never been substituted in
/// is wired by what its own manifest says, which is the state every stack starts in.
pub(crate) fn chosen_fillers(ctx: &Ctx) -> crate::wiring::Chosen {
    crate::wiring::Chosen::read(recorded_secret(ctx, crate::wiring::FILLS_KEY).as_deref())
        .because(recorded_secret(ctx, crate::wiring::FILLS_WHY_KEY).as_deref())
}

/// The write side of [`recorded_secret`]: record a credential where a later run — and
/// the dashboard — reads it back.
///
/// A failure is the caller's to act on rather than this one's to swallow. A secret
/// lemonfiber mints is recorded *before* any service is given it, because the record
/// is the only copy lemonfiber will ever have: a service holding a password nothing
/// recorded is a service lemonfiber is locked out of. So a machine with no file to
/// keep it in is a failure too.
///
/// # Errors
///
/// Returns the [`store::Failure`] that stopped it being recorded, or
/// [`store::Failure::Nowhere`] where there is no environment file.
pub(crate) fn record_secret(ctx: &Ctx, key: &str, value: &str) -> Result<(), store::Failure> {
    let path = ctx
        .settings
        .env_file
        .as_deref()
        .ok_or(store::Failure::Nowhere)?;
    store::set(path, key, value)
}

/// What the secret store holds under `name`, where it holds a value.
pub(crate) fn stored_secret(ctx: &Ctx, name: &str) -> Option<String> {
    crate::settings::secrets_at(ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .and_then(|secrets| secrets.value(name).map(str::to_owned))
}

/// The key lemonfiber minted for the stack's service `service` where a first-party part
/// moved its own service in place of it: the secret the adapter in front of that service
/// is handed in its `upstream.json`, as the secret store holds it.
pub(crate) fn moved_credential(ctx: &Ctx, service: &str) -> Option<String> {
    let register = crate::app::plugins::read(ctx).ok()?;
    let installed = crate::project::moving(
        register.installed(),
        service,
        crate::plugin::first_party::EMBEDDED,
    )?;
    let secret = installed
        .services
        .iter()
        .filter(|placed| placed.fronts.as_deref() == Some(service))
        .flat_map(|placed| placed.upstream.values())
        .find(|named| {
            installed
                .declared
                .secrets
                .iter()
                .any(|one| one.minted && one.of == service && one.id == **named)
        })?;
    stored_secret(ctx, &crate::settings::named(&installed.plugin, secret))
}

#[cfg(test)]
mod tests;
