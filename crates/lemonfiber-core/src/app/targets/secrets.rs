//! The secrets lemonfiber minted rather than read.
//!
//! Almost every credential in the stack is a service's own, read from where it wrote it.
//! The exceptions are recorded beside the environment file, because there is nowhere else
//! they could be read back from.

use crate::app::Ctx;
use crate::config::store;
use crate::journal::{Change, Kind};
use crate::settings::legacy::held_as;
use crate::settings::{Keeping, Source};

/// What lemonfiber recorded under `key`, or nothing where nothing is recorded yet: a
/// credential from the secret store, under the name it holds it as, and anything else
/// from the environment file. The one reader for every credential lemonfiber records
/// rather than reads from a service, so the read-back stays identical across them.
///
/// A credential still in the environment file, where a run has not filed it yet, as on a
/// rehearsal before the first run that does, is read there until it is. A credential
/// named for what it reaches is read under the name an earlier release gave it while it
/// has not moved yet.
pub(crate) fn recorded_secret(ctx: &Ctx, key: &str) -> Option<String> {
    let file = ctx
        .settings
        .env_file
        .as_deref()
        .map(|path| store::read(path).unwrap_or_default())
        .unwrap_or_default();
    let held = |key: &str| {
        held_as(key)
            .and_then(|name| stored_secret(ctx, &name))
            .or_else(|| file.get(key).map(str::to_owned))
            .filter(|value| !value.is_empty())
    };
    held(key).or_else(|| earlier_credential(ctx, key).and_then(|earlier| held(&earlier)))
}

/// The name an earlier release recorded the credential the stack now keeps as `key`
/// under, where `key` names one.
fn earlier_credential(ctx: &Ctx, key: &str) -> Option<String> {
    let replacing = key.strip_suffix(crate::config::PENDING_SUFFIX);
    let named = replacing.unwrap_or(key);
    let endings: Vec<&str> = crate::config::CREDENTIAL_SUFFIXES
        .into_iter()
        .filter(|ending| named.ends_with(ending))
        .collect();
    if endings.is_empty() {
        return None;
    }
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    let fillers = super::fillers_here(ctx, &manifest);
    let earlier = fillers.services().find_map(|filler| {
        endings
            .iter()
            .find(|ending| fillers.setting(filler, ending).as_deref() == Some(named))
            .and_then(|ending| crate::wiring::Fillers::earlier(filler, ending))
    })?;
    Some(if replacing.is_some() {
        format!("{earlier}{}", crate::config::PENDING_SUFFIX)
    } else {
        earlier
    })
}

/// The credential lemonfiber records for the service `service` of the stack here, ending
/// in `holds`: under the name the whole stack gives it, which is the one it is written
/// under, or under the name an earlier release gave it until it has moved.
pub(crate) fn recorded_credential(ctx: &Ctx, service: &str, holds: &str) -> Option<String> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    let fillers = super::fillers_here(ctx, &manifest);
    let filler = fillers.service(service)?;
    fillers
        .setting(filler, holds)
        .and_then(|named| recorded_secret(ctx, &named))
        .or_else(|| recorded_secret(ctx, &crate::wiring::Fillers::earlier(filler, holds)?))
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

/// The parts the operator's choices of filler take out of their form, as
/// [`crate::wiring::replaced`] answers over `manifest` and what is installed.
pub(crate) fn replaced_parts(ctx: &Ctx, manifest: &lemonfiber_manifest::Manifest) -> Vec<String> {
    let installed = crate::app::plugins::read(ctx).unwrap_or_default();
    crate::wiring::replaced(manifest, installed.installed(), &chosen_fillers(ctx))
}

/// The ordinal each service holds where two would share a credential's name, as the
/// environment file records them.
pub(crate) fn ordinals(ctx: &Ctx) -> crate::config::credentials::Ordinals {
    crate::config::credentials::Ordinals::read(
        recorded_secret(ctx, crate::config::credentials::ORDINALS_KEY).as_deref(),
    )
}

/// The write side of [`recorded_secret`]: the credential lemonfiber minted for `key`, held
/// in the secret store under the name it holds it as, and journalled under `operation` by
/// the fingerprints of the value it replaces and of itself.
///
/// A failure is the caller's to act on rather than this one's to swallow. A secret
/// lemonfiber mints is recorded *before* any service is given it, because the record
/// is the only copy lemonfiber will ever have: a service holding a password nothing
/// recorded is a service lemonfiber is locked out of. So a machine with no store to
/// keep it in is a failure too.
///
/// # Errors
///
/// Returns the [`store::Failure`] that stopped it being recorded, or
/// [`store::Failure::Nowhere`] where there is no store.
pub(crate) fn record_secret(
    ctx: &Ctx,
    key: &str,
    value: &str,
    operation: &str,
) -> Result<(), store::Failure> {
    let (secrets, journal) = held_where(ctx)?;
    let name = held_as(key).unwrap_or_else(|| key.to_owned());
    let stamp = ctx.stamp();
    let keeping = [Keeping {
        name: &name,
        value,
        from: Source::Minted,
        at: ctx.seconds(),
    }];
    crate::settings::kept(&secrets, &keeping, ctx.seams.random.as_ref(), &|replaced| {
        let changes: Vec<Change> = replaced
            .iter()
            .map(|one| {
                secret_changed(
                    &one.name,
                    one.previous.clone(),
                    one.current.clone(),
                    operation,
                    &stamp,
                )
            })
            .collect();
        crate::app::recover::journalled(&journal, &changes)
    })?;
    unfiled_where(ctx, key)
}

/// The line an earlier release wrote for the credential `key` taken out of the
/// environment file, where there is one.
fn unfiled_where(ctx: &Ctx, key: &str) -> Result<(), store::Failure> {
    ctx.settings
        .env_file
        .as_deref()
        .map_or(Ok(()), |env| crate::settings::legacy::unfiled(env, key))
}

/// The credential recorded for `key` forgotten from the secret store, journalled under
/// `operation` by the fingerprint of the value it held, and from the environment file
/// where an earlier release left it there.
///
/// # Errors
///
/// Returns the [`store::Failure`] that stopped it being forgotten, or
/// [`store::Failure::Nowhere`] where there is no store.
pub(crate) fn forget_secret(ctx: &Ctx, key: &str, operation: &str) -> Result<(), store::Failure> {
    unfiled_where(ctx, key)?;
    let (secrets, journal) = held_where(ctx)?;
    let name = held_as(key).unwrap_or_else(|| key.to_owned());
    if crate::settings::Secrets::at(&secrets)?
        .value(&name)
        .is_none()
    {
        return Ok(());
    }
    let previous =
        crate::settings::keyed_at(&secrets, ctx.seams.random.as_ref())?.fingerprint(&name);
    let change = secret_changed(&name, previous, None, operation, &ctx.stamp());
    crate::app::recover::journalled(&journal, &[change])?;
    crate::settings::dropped(&secrets, &[name])
}

/// The secret store and the journal beside the environment file.
fn held_where(ctx: &Ctx) -> Result<(std::path::PathBuf, std::path::PathBuf), store::Failure> {
    let secrets = crate::settings::secrets_at(ctx).ok_or(store::Failure::Nowhere)?;
    let journal = secrets.with_file_name(crate::config::paths::JOURNAL);
    Ok((secrets, journal))
}

/// The journal entry for the secret `name` going from the value fingerprinted `previous`
/// to the one fingerprinted `current`, minted by lemonfiber.
fn secret_changed(
    name: &str,
    previous: Option<String>,
    current: Option<String>,
    operation: &str,
    stamp: &str,
) -> Change {
    Change {
        at: stamp.to_owned(),
        operation: operation.to_owned(),
        target: name.to_owned(),
        kind: Kind::Secret {
            name: name.to_owned(),
            from: Source::Minted.as_str().to_owned(),
            previous,
            current,
            filed: None,
        },
    }
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
