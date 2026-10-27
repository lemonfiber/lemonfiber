//! Moving the values an earlier release kept under a part's name to the keys that say
//! what they hold.
//!
//! The media server's mode moves by a name this build holds. A credential moves from the
//! name an earlier release spelled from its service's id to the one spelled from the
//! capability it reaches, which only the stack it is in can say, so each is worked out
//! from who fills what here, after the ordinal of every service sharing a name with
//! another is recorded. A grant naming a credential by its earlier name is
//! rewritten with it.

use crate::app::Ctx;
use crate::config::credentials::{Ordinals, ORDINALS_KEY};
use crate::config::renamed::RENAMED;
use crate::config::store::{self, Failure};
use crate::config::{CREDENTIAL_SUFFIXES, PENDING_SUFFIX};
use crate::journal::{Change, Kind};
use crate::wiring::Fillers;

/// Move each value the environment file holds under a key an earlier release wrote to the
/// key it is kept under now, journalled, leaving one whose key now holds a value as it is,
/// and an empty one where it stands.
///
/// # Errors
///
/// Where the environment file, the record of what is installed or the journal cannot be
/// read or written.
pub(crate) fn renamed(ctx: &Ctx) -> Result<(), Failure> {
    if ctx.dry_run {
        return Ok(());
    }
    let Some(env) = ctx.settings.env_file.as_deref() else {
        return Ok(());
    };
    let file = store::read(env)?;
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    manifest.iter().try_for_each(|manifest| {
        numbered(
            ctx,
            env,
            &file,
            &crate::app::targets::fillers_here(ctx, manifest),
        )
    })?;
    let pairs: Vec<(String, String)> =
        RENAMED
            .iter()
            .map(|(was, now)| ((*was).to_owned(), (*now).to_owned()))
            .chain(manifest.iter().flat_map(|manifest| {
                credentials(&crate::app::targets::fillers_here(ctx, manifest))
            }))
            .collect();
    let held = |key: &str| file.written(key).filter(|value| !value.is_empty());
    let moving: Vec<(&str, &str, &str)> = pairs
        .iter()
        .filter_map(|(was, now)| match (held(was), held(now)) {
            (Some(value), None) => Some((was.as_str(), now.as_str(), value)),
            _ => None,
        })
        .collect();
    if !moving.is_empty() {
        let changes: Vec<Change> = moving
            .iter()
            .map(|(was, now, _)| {
                change(
                    ctx,
                    now,
                    Kind::Renamed {
                        from: (*was).to_owned(),
                        to: (*now).to_owned(),
                    },
                )
            })
            .collect();
        journalled(ctx, &changes)?;
        for (was, now, value) in &moving {
            moved(env, was, now, value)?;
        }
    }
    regranted(ctx, env, &pairs)
}

/// Every name two credentials of the stack here would both take.
pub(crate) fn clashing(ctx: &Ctx) -> Vec<String> {
    ctx.stack
        .checked_manifest(ctx.today())
        .map(|manifest| crate::app::targets::fillers_here(ctx, &manifest).clashing())
        .unwrap_or_default()
}

/// Every credential of a service here whose name an earlier release spelled from the
/// service's id, as that name and the one it is kept under now, a rotation's pending
/// replacement's beside each.
fn credentials(fillers: &Fillers) -> Vec<(String, String)> {
    fillers
        .services()
        .flat_map(|filler| {
            CREDENTIAL_SUFFIXES.into_iter().filter_map(move |holds| {
                let was = crate::wiring::Fillers::earlier(filler, holds)?;
                let now = fillers.setting(filler, holds)?;
                (was != now).then_some((was, now))
            })
        })
        .flat_map(|(was, now)| {
            let pending = (
                format!("{was}{PENDING_SUFFIX}"),
                format!("{now}{PENDING_SUFFIX}"),
            );
            [(was, now), pending]
        })
        .collect()
}

/// Record the ordinal of every service sharing a capability with another, where it is
/// not recorded yet and a service of that capability holds a credential under the name
/// an earlier release gave it, journalled.
fn numbered(
    ctx: &Ctx,
    env: &std::path::Path,
    file: &crate::config::env::EnvFile,
    fillers: &Fillers,
) -> Result<(), Failure> {
    let mut ordinals: Ordinals = fillers.ordinals().clone();
    for holds in CREDENTIAL_SUFFIXES {
        let numbering = fillers.numbering(holds);
        let held = |service: &str| {
            [holds.to_owned(), format!("{holds}{PENDING_SUFFIX}")]
                .iter()
                .any(|ending| {
                    file.written(&crate::config::for_service(service, ending))
                        .is_some_and(|value| !value.is_empty())
                })
        };
        for (service, capability, ordinal) in &numbering {
            let holding = numbering
                .iter()
                .any(|(other, shared, _)| shared == capability && held(other));
            if holding && ordinals.of(service, capability).is_none() {
                ordinals.record(service, capability, *ordinal);
            }
        }
    }
    if crate::app::targets::layout(ctx).is_none() {
        return Ok(());
    }
    if &ordinals == fillers.ordinals() {
        return Ok(());
    }
    let previous = file.written(ORDINALS_KEY).map(str::to_owned);
    let current = ordinals.written();
    journalled(
        ctx,
        &[change(
            ctx,
            ORDINALS_KEY,
            Kind::Set {
                key: ORDINALS_KEY.to_owned(),
                previous,
                current: current.clone(),
            },
        )],
    )?;
    store::set(env, ORDINALS_KEY, &current)
}

/// Rewrite every grant naming a credential by an earlier name of `pairs` to the name it
/// is kept under now, journalled so undoing it writes the earlier record back.
fn regranted(ctx: &Ctx, env: &std::path::Path, pairs: &[(String, String)]) -> Result<(), Failure> {
    let at = env.with_file_name(crate::config::paths::PLUGINS);
    let Ok(previous) = std::fs::read_to_string(&at) else {
        return Ok(());
    };
    let Ok(mut register) = crate::plugin::Register::parse(&previous) else {
        return Ok(());
    };
    let renamed = pairs
        .iter()
        .filter(|(was, now)| register.regranted_as(was, now))
        .count();
    let Some(text) = register.to_json().filter(|_| renamed > 0) else {
        return Ok(());
    };
    journalled(
        ctx,
        &[crate::app::project::change(
            &at,
            Some(previous),
            &text,
            super::OPERATION,
            &ctx.stamp(),
        )],
    )?;
    store::write(&at, &text)
}

/// A change of this pass's, made now.
fn change(ctx: &Ctx, target: &str, kind: Kind) -> Change {
    Change {
        at: ctx.stamp(),
        operation: super::OPERATION.to_owned(),
        target: target.to_owned(),
        kind,
    }
}

/// The value `was` held kept under `now` in the environment file at `env`, and `was` gone.
fn moved(env: &std::path::Path, was: &str, now: &str, value: &str) -> Result<(), Failure> {
    store::set(env, now, value)?;
    store::unset(env, was)
}

/// Write `changes` to the journal.
fn journalled(ctx: &Ctx, changes: &[Change]) -> Result<(), Failure> {
    let paths = crate::app::targets::layout(ctx).ok_or(Failure::Nowhere)?;
    crate::app::recover::journalled(&paths.journal(), changes, ctx.seams.random.as_ref())
}

#[cfg(test)]
mod tests;
