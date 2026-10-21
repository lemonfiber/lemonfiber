use crate::app::Ctx;
use crate::config::paths::{SECRETS, SETTINGS};
use crate::config::storage::{self, Storage};
use crate::config::store::{self, Failure};
use crate::config::{DATA_ROOT_KEY, NAS_EXPORT_KEY, NAS_HOST_KEY};
use crate::credential::fingerprint;
use crate::journal::{Change, Kind};
use crate::plugin::Installed;

use super::legacy::{self, Legacy};
use super::secrets::Keeping;
use super::{Settings, Source};

struct Filing {
    name: String,
    key: String,
    value: String,
}

/// File what the operator's environment file and the stack's hold into the stores, saying
/// so where the stack's could not be filed yet.
pub(crate) async fn everything(ctx: &Ctx) {
    filed(ctx);
    if let Err(failure) = stack(ctx) {
        ctx.narrator
            .say(&format!(
                "the stack's settings could not be moved into lemonfiber's own stores yet: {failure}"
            ))
            .await;
    }
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
    let installed = crate::app::plugins::read(ctx)
        .map(|register| register.installed().to_vec())
        .unwrap_or_default();
    let filings = candidates(ctx, &installed);
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
    super::secrets::filed(&env.with_file_name(SECRETS), &keeping, &|absent| {
        let changes: Vec<Change> = absent
            .iter()
            .map(|one| change(one, &filings, &stamp))
            .collect();
        crate::app::recover::journalled(&paths.journal(), &changes, ctx.seams.random.as_ref())
    })?;
    for filing in &filings {
        store::unset(env, &filing.key)?;
    }
    let touched: Vec<&Installed> = installed
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

/// Moves every value the stack's environment file holds for a part or a published setting
/// into the stores, once.
///
/// # Errors
///
/// Where a store, the journal or the environment file cannot be read or written.
pub fn stack(ctx: &Ctx) -> Result<(), Failure> {
    if ctx.dry_run {
        return Ok(());
    }
    let Some(env) = ctx.settings.env_file.as_deref() else {
        return Ok(());
    };
    let settings = env.with_file_name(SETTINGS);
    if Settings::at(&settings)?.is_stack_filed() {
        return Ok(());
    }
    let file = store::read(env)?;
    let rows: Vec<(&Legacy, &str)> = legacy::TABLE
        .iter()
        .filter_map(|row| {
            file.get(row.key)
                .filter(|value| !value.is_empty())
                .map(|value| (row, value))
        })
        .collect();
    if !rows.is_empty() {
        let paths = crate::app::targets::layout(ctx).ok_or(Failure::Nowhere)?;
        let journal = paths.journal();
        let at = ctx.seconds();
        let stamp = ctx.stamp();
        let keeping: Vec<Keeping<'_>> = rows
            .iter()
            .filter(|(row, _)| row.secret)
            .map(|(row, value)| Keeping {
                name: row.name,
                value,
                from: Source::Operator,
                at,
            })
            .collect();
        super::secrets::filed(&env.with_file_name(SECRETS), &keeping, &|absent| {
            let changes: Vec<Change> = absent
                .iter()
                .map(|one| filed_secret(one, legacy::named(one.name), &stamp))
                .collect();
            crate::app::recover::journalled(&journal, &changes, ctx.seams.random.as_ref())
        })?;
        let filing: Vec<super::store::Filing<'_>> = rows
            .iter()
            .filter(|(row, _)| !row.secret)
            .map(|(row, value)| (row.name, *value))
            .collect();
        super::store::filed(&settings, &filing, &|absent| {
            let changes: Vec<Change> = absent
                .iter()
                .map(|(name, value)| filed_setting(name, value, &stamp))
                .collect();
            crate::app::recover::journalled(&journal, &changes, ctx.seams.random.as_ref())
        })?;
        for (row, _) in &rows {
            store::unset(env, row.key)?;
        }
    }
    nas_folded(ctx, env, &file)?;
    super::store::marked_stack_filed(&settings)
}

fn nas_folded(
    ctx: &Ctx,
    env: &std::path::Path,
    file: &crate::config::env::EnvFile,
) -> Result<(), Failure> {
    let held = |key: &str| {
        file.get(key)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };
    let (Some(host), Some(path)) = (held(NAS_HOST_KEY), held(NAS_EXPORT_KEY)) else {
        return Ok(());
    };
    let root = held(DATA_ROOT_KEY);
    if root.is_some_and(|root| root.starts_with(storage::NFS)) {
        return Ok(());
    }
    let Ok(Some(Storage::Nfs(export))) = Storage::read(&format!("{}{host}{path}", storage::NFS))
    else {
        return Ok(());
    };
    let folded = export.written();
    let stamp = ctx.stamp();
    let set = |key: &str, previous: Option<&str>, current: &str| Change {
        at: stamp.clone(),
        operation: super::OPERATION.to_owned(),
        target: key.to_owned(),
        kind: Kind::Set {
            key: key.to_owned(),
            previous: previous.map(str::to_owned),
            current: current.to_owned(),
        },
    };
    let changes = [
        set(DATA_ROOT_KEY, root, &folded),
        set(NAS_HOST_KEY, Some(host), ""),
        set(NAS_EXPORT_KEY, Some(path), ""),
    ];
    let paths = crate::app::targets::layout(ctx).ok_or(Failure::Nowhere)?;
    crate::app::recover::journalled(&paths.journal(), &changes, ctx.seams.random.as_ref())?;
    store::set(env, DATA_ROOT_KEY, &folded)?;
    store::set(env, NAS_HOST_KEY, "")?;
    store::set(env, NAS_EXPORT_KEY, "")
}

fn filed_secret(one: &Keeping<'_>, row: Option<&Legacy>, stamp: &str) -> Change {
    Change {
        at: stamp.to_owned(),
        operation: super::OPERATION.to_owned(),
        target: one.name.to_owned(),
        kind: Kind::Secret {
            name: one.name.to_owned(),
            from: one.from.as_str().to_owned(),
            previous: None,
            current: Some(fingerprint(one.value)),
            filed: row.map(|row| row.key.to_owned()),
        },
    }
}

fn filed_setting(name: &str, value: &str, stamp: &str) -> Change {
    Change {
        at: stamp.to_owned(),
        operation: super::OPERATION.to_owned(),
        target: name.to_owned(),
        kind: Kind::Setting {
            name: name.to_owned(),
            previous: None,
            current: Some(value.to_owned()),
            filed: legacy::setting_key(name).map(str::to_owned),
        },
    }
}
