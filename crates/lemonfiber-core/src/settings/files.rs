use crate::app::Ctx;
use crate::config::env::quoted;
use crate::config::store::{self, Failure};
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

pub(crate) fn forgotten(ctx: &Ctx, plugin: &Installed) {
    if let Some(files) = super::resolved::files_of(ctx) {
        for placed in &plugin.services {
            let _ = std::fs::remove_file(super::resolved::file_in(&files, &placed.service));
        }
    }
}
