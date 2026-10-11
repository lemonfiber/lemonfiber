use crate::app::Ctx;
use crate::config::env::quoted;
use crate::config::paths::SECRETS;
use crate::config::store::{self, Failure};
use crate::plugin::{Holding, Installed, Variable};

use super::{Resolved, Secrets};

pub(crate) fn text(ctx: &Ctx, variables: &[Variable]) -> Result<String, Failure> {
    let secrets = match ctx.settings.env_file.as_deref() {
        Some(env) => Secrets::at(&env.with_file_name(SECRETS))?,
        None => Secrets::default(),
    };
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
    let resolved = Resolved::of(ctx, manifest.as_ref(), &beside);
    let planned = resolved.reading(&|_, _| None, |reading| {
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
    let Some(files) = super::resolved::files_of(ctx) else {
        return;
    };
    for placed in &plugin.services {
        let _ = std::fs::remove_file(super::resolved::file_in(&files, &placed.service));
    }
}
