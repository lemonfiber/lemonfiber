use std::path::PathBuf;

use lemonfiber_manifest::container::Value;
use lemonfiber_manifest::{Manifest, Service};

use crate::app::Ctx;
use crate::config::env::quoted;
use crate::config::store::{self, Failure};
use crate::error::{Diagnose as _, Problem};
use crate::plugin::{Holding, Installed, Variable};

use super::{Resolved, Secrets};

pub(crate) fn text(ctx: &Ctx, variables: &[Variable]) -> Result<String, Failure> {
    let secrets = super::secrets_at(ctx)
        .map(|at| Secrets::at(&at))
        .transpose()?
        .unwrap_or_default();
    Ok(variables
        .iter()
        .filter_map(|variable| {
            let value = match &variable.holding {
                Holding::Store(name) => secrets.value(name).map(str::to_owned),
                Holding::Crossing(setting) => crate::app::targets::recorded_secret(ctx, setting),
                Holding::Withheld => None,
            }?;
            crate::config::env::is_one_line(&value)
                .then(|| format!("{}={}\n", variable.name, quoted::written(&value)))
        })
        .collect())
}

pub(crate) fn written(ctx: &Ctx, plugins: &[&Installed]) -> Result<(), Failure> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    let beside: Vec<Installed> = crate::app::plugins::read(ctx)
        .map(|register| register.installed().to_vec())
        .unwrap_or_default()
        .into_iter()
        .filter(|one| plugins.iter().all(|plugin| plugin.plugin != one.plugin))
        .chain(plugins.iter().map(|plugin| (*plugin).clone()))
        .collect();
    written_by(ctx, plugins, &Resolved::of(ctx, manifest.as_ref(), &beside))
}

pub(crate) fn written_by(
    ctx: &Ctx,
    plugins: &[&Installed],
    resolved: &Resolved,
) -> Result<(), Failure> {
    let planned = resolved.reading(|reading| {
        plugins
            .iter()
            .flat_map(|plugin| {
                plugin.services.iter().filter_map(move |placed| {
                    crate::plugin::secrets_file(placed, reading)
                        .map(|path| (path, crate::plugin::variables(plugin, placed, reading)))
                })
            })
            .collect::<Vec<_>>()
    });
    for (path, variables) in planned {
        store::write(&path, &text(ctx, &variables)?)?;
    }
    Ok(())
}

/// Where a secrets file reads the value `key` names in the environment file from: the
/// secret store where the key is one filed there, and the credential itself otherwise.
pub(crate) fn holding_of(key: &str) -> Holding {
    match super::legacy::keyed(key) {
        Some(row) if row.secret => Holding::Store(row.name.to_owned()),
        _ => Holding::Crossing(key.to_owned()),
    }
}

/// Every secrets file, the stack's own services' and each of `installed`'s, by the
/// service it is of, with every variable it holds.
pub(crate) fn every_file(
    manifest: Option<&Manifest>,
    installed: &[Installed],
    resolved: &Resolved,
) -> Vec<(String, PathBuf, Vec<Variable>)> {
    let mut files: Vec<(String, PathBuf, Vec<Variable>)> = manifest
        .into_iter()
        .flat_map(|manifest| crate::project::written(manifest, installed))
        .filter(|service| crate::project::names_a_secret(&service.env))
        .filter_map(|service| {
            resolved.secrets_file(&service.id).map(|path| {
                (
                    service.id.clone(),
                    path,
                    of_stack_service(service, resolved),
                )
            })
        })
        .collect();
    files.extend(resolved.reading(|reading| {
        installed
            .iter()
            .flat_map(|plugin| {
                plugin.services.iter().filter_map(move |placed| {
                    crate::plugin::secrets_file(placed, reading).map(|path| {
                        (
                            placed.service.clone(),
                            path,
                            crate::plugin::variables(plugin, placed, reading),
                        )
                    })
                })
            })
            .collect::<Vec<_>>()
    }));
    files
}

/// Rewrite every secrets file that reads `holding`, each value read from where it is held:
/// what a change to that value owes every entry that reads it. Answers each service whose
/// file was rewritten.
///
/// # Errors
///
/// Where a store cannot be read or a file cannot be written.
pub(crate) fn reread(
    ctx: &Ctx,
    manifest: Option<&Manifest>,
    installed: &[Installed],
    resolved: &Resolved,
    holding: &Holding,
) -> Result<Vec<String>, Failure> {
    let mut rewritten = Vec::new();
    for (service, path, variables) in every_file(manifest, installed, resolved) {
        if variables
            .iter()
            .any(|variable| variable.holding == *holding)
        {
            store::write(&path, &text(ctx, &variables)?)?;
            rewritten.push(service);
        }
    }
    Ok(rewritten)
}

/// What is said where a value that changed could not be written into a file that reads it.
pub(crate) const UNREACHED: &str = "The value itself changed. A service reading it from this \
                                    file still holds the one it replaced until the file is \
                                    written again.";

/// [`reread`] over the stack and everything installed as they stand, once the value
/// `holding` names has changed.
///
/// # Errors
///
/// Where a store cannot be read or a file cannot be written, saying that the value changed
/// all the same.
pub(crate) fn reread_now(ctx: &Ctx, holding: &Holding) -> Result<(), Box<Problem>> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    let installed: Vec<Installed> = crate::app::plugins::read(ctx)
        .map(|register| register.installed().to_vec())
        .unwrap_or_default();
    let resolved = Resolved::of(ctx, manifest.as_ref(), &installed);
    reread(ctx, manifest.as_ref(), &installed, &resolved, holding)
        .map(drop)
        .map_err(|failure| {
            Box::new(Problem {
                meaning: UNREACHED.to_owned(),
                ..failure.problem()
            })
        })
}

/// Every variable of a stack service's secrets file, each with where its value is read
/// from.
pub(crate) fn of_stack_service(service: &Service, resolved: &Resolved) -> Vec<Variable> {
    service
        .env
        .iter()
        .filter_map(|(name, value)| match value {
            Value::Secret(secret) => Some(Variable {
                name: name.clone(),
                holding: stack_holding(service, &secret.secret, resolved),
            }),
            _ => None,
        })
        .collect()
}

/// Where a stack service's secret is read from: the secret store, where the operator
/// enters it; the credential lemonfiber keeps for the service itself, under the stack's
/// own name for it; and a credential that crosses from another service, where its holder
/// keeps it, read there each time and never copied.
pub(crate) fn stack_holding(service: &Service, id: &str, resolved: &Resolved) -> Holding {
    let named = crate::settings::named(&service.id, id);
    if crate::settings::legacy::named(&named).is_some_and(|row| row.secret) {
        return Holding::Store(named);
    }
    match service.secret.iter().find(|secret| secret.id == id) {
        Some(secret) if secret.of != service.id => resolved.crossing_into(&service.id, &secret.of),
        _ => Holding::Crossing(crate::config::for_service(
            &service.id,
            crate::config::API_KEY_SUFFIX,
        )),
    }
}

pub(crate) fn forgotten(ctx: &Ctx, plugin: &Installed) {
    if let Some(files) = super::resolved::files_of(ctx) {
        for placed in &plugin.services {
            let _ = std::fs::remove_file(super::resolved::file_in(&files, &placed.service));
        }
    }
}
