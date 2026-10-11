use crate::app::Ctx;
use crate::config::paths::{SECRETS, SETTINGS};
use crate::config::store::{self, Failure};
use crate::credential::fingerprint;
use crate::journal::{Change, Kind};
use crate::plugin::Installed;

use super::secrets::Keeping;
use super::{Settings, Source};

struct Filing {
    name: String,
    key: String,
    value: String,
}

pub(crate) fn filed(ctx: &Ctx) {
    if ctx.dry_run {
        return;
    }
    let _ = filing(ctx);
}

fn filing(ctx: &Ctx) -> Result<(), Failure> {
    let Some(env) = ctx.settings.env_file.as_deref() else {
        return Ok(());
    };
    if Settings::at(&env.with_file_name(SETTINGS))?.is_filed() {
        return Ok(());
    }
    let Ok(register) = crate::app::plugins::read(ctx) else {
        return Ok(());
    };
    let filings = candidates(ctx, register.installed());
    if filings.is_empty() {
        return Ok(());
    }
    let paths = crate::app::targets::layout(ctx).ok_or(Failure::Nowhere)?;
    let at = ctx.seconds();
    let stamp = ctx.stamp();
    let keeping: Vec<Keeping<'_>> = filings
        .iter()
        .map(|filing| Keeping {
            name: &filing.name,
            value: &filing.value,
            from: Source::Recipe,
            at,
        })
        .collect();
    super::secrets::filed(&env.with_file_name(SECRETS), &keeping, |absent| {
        let changes: Vec<Change> = absent
            .iter()
            .map(|one| change(one, &filings, &stamp))
            .collect();
        crate::app::recover::journalled(&paths.journal(), &changes, ctx.seams.random.as_ref())
    })?;
    for filing in &filings {
        store::unset(env, &filing.key)?;
    }
    let touched: Vec<&Installed> = register
        .installed()
        .iter()
        .filter(|one| {
            filings
                .iter()
                .any(|filing| super::in_namespace(&filing.name, &one.plugin))
        })
        .collect();
    super::written(ctx, &touched)?;
    super::store::marked_filed(&env.with_file_name(SETTINGS))
}

fn candidates(ctx: &Ctx, installed: &[Installed]) -> Vec<Filing> {
    installed
        .iter()
        .flat_map(|one| {
            one.declared.secrets.iter().filter_map(|secret| {
                let key = crate::config::for_plugin(
                    &one.plugin,
                    &secret.id,
                    crate::config::CAPTURED_SUFFIX,
                );
                crate::app::targets::recorded_secret(ctx, &key).map(|value| Filing {
                    name: super::named(&one.plugin, &secret.id),
                    key,
                    value,
                })
            })
        })
        .collect()
}

fn change(one: &Keeping<'_>, filings: &[Filing], stamp: &str) -> Change {
    Change {
        at: stamp.to_owned(),
        operation: super::OPERATION.to_owned(),
        target: one.name.to_owned(),
        kind: Kind::Secret {
            name: one.name.to_owned(),
            from: one.from.as_str().to_owned(),
            previous: None,
            current: Some(fingerprint(one.value)),
            filed: filings
                .iter()
                .find(|filing| filing.name == one.name)
                .map(|filing| filing.key.clone()),
        },
    }
}
