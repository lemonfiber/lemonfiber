use std::path::Path;

use lemonfiber_manifest::container::earlier::{self, Earlier};

use crate::app::Ctx;
use crate::config::env::EnvFile;
use crate::config::paths::{JOURNAL, SECRETS, SETTINGS};
use crate::config::storage::{self, Storage};
use crate::config::store::{self, Failure};
use crate::config::{DATA_ROOT_KEY, NAS_EXPORT_KEY, NAS_HOST_KEY};
use crate::journal::{Change, Kind};
use crate::plugin::first_party::{holds, EMBEDDED};
use crate::plugin::Installed;

use super::legacy;
use super::secrets::Keeping;
use super::{Secrets, Settings, Source};

struct Filing {
    name: String,
    key: String,
    value: String,
}

/// File what the operator's environment file and the stack's hold into the stores, saying
/// so where the stack's could not be filed yet.
pub async fn everything(ctx: &Ctx) {
    if let Err(failure) = super::renaming::renamed(ctx) {
        ctx.narrator
            .say(&format!(
                "the settings an earlier release kept under a part's name could not be moved \
                 to their own names yet: {failure}"
            ))
            .await;
    }
    for name in super::renaming::clashing(ctx) {
        ctx.narrator
            .say(&format!(
                "two services' credentials would both be kept as {name}, so neither is \
                 kept or read under it"
            ))
            .await;
    }
    filed(ctx);
    if let Err(failure) = stack(ctx).and_then(|()| credentials(ctx)) {
        ctx.narrator
            .say(&format!(
                "the stack's settings could not be moved into lemonfiber's own stores yet: {failure}"
            ))
            .await;
    }
}

/// File every credential the environment file still holds into the secret store, under
/// the name the store holds it as, journalled by its keyed fingerprint, and take it out of the
/// file: one the store already holds a value for leaves the file without being kept.
///
/// Asked on every run rather than once, so a credential an earlier release wrote to the
/// file after its settings were filed is filed too, and the file is never read for one.
pub(crate) fn credentials(ctx: &Ctx) -> Result<(), Failure> {
    if ctx.dry_run {
        return Ok(());
    }
    let Some(env) = ctx.settings.env_file.as_deref() else {
        return Ok(());
    };
    let file = store::read(env)?;
    let held = credentials_in(&file);
    if held.is_empty() {
        return Ok(());
    }
    let journal = env.with_file_name(JOURNAL);
    let (at, stamp) = (ctx.seconds(), ctx.stamp());
    let keeping: Vec<Keeping<'_>> = held
        .iter()
        .map(|(key, name, value)| Keeping {
            name,
            value,
            from: legacy::source_of(key),
            at,
        })
        .collect();
    let random = ctx.seams.random.as_ref();
    super::secrets::filed(&env.with_file_name(SECRETS), &keeping, random, &|absent| {
        let changes: Vec<Change> = absent
            .iter()
            .map(|(one, current)| {
                let key = held
                    .iter()
                    .find(|(_, name, _)| name == one.name)
                    .map(|(key, _, _)| key.clone());
                filed_secret(one, current.clone(), key, &stamp)
            })
            .collect();
        crate::app::recover::journalled(&journal, &changes)
    })?;
    for (key, _, _) in &held {
        store::unset(env, key)?;
    }
    Ok(())
}

/// Every credential `file` holds, as the key it is written under, the name the secret
/// store holds it as and its value.
fn credentials_in(file: &EnvFile) -> Vec<(String, String, &str)> {
    file.keys()
        .into_iter()
        .filter_map(|key| {
            let value = file.get(key).filter(|value| !value.is_empty())?;
            Some((key.to_owned(), legacy::held_as(key)?, value))
        })
        .collect()
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
    let filings = candidates(env, &store::read(env)?, &installed);
    if filings.is_empty() {
        return Ok(());
    }
    let journal = env.with_file_name(JOURNAL);
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
    let random = ctx.seams.random.as_ref();
    super::secrets::filed(&env.with_file_name(SECRETS), &keeping, random, &|absent| {
        let changes: Vec<Change> = absent
            .iter()
            .map(|(one, current)| change(one, current.clone(), &filings, &stamp))
            .collect();
        crate::app::recover::journalled(&journal, &changes)
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

/// Every secret of `installed` an earlier release kept under its captured key, in the
/// secret store or in `file`, the environment file at `env`.
fn candidates(env: &Path, file: &EnvFile, installed: &[Installed]) -> Vec<Filing> {
    let kept = Secrets::at(&env.with_file_name(SECRETS)).unwrap_or_default();
    installed
        .iter()
        .flat_map(|one| {
            let services = || crate::plugin::services_of(one);
            one.declared
                .secrets
                .iter()
                .filter(move |secret| {
                    crate::plugin::own_secret(&one.plugin, services(), &secret.of)
                })
                .filter_map(|secret| {
                    let key = crate::config::for_plugin(
                        &one.plugin,
                        &secret.id,
                        crate::config::CAPTURED_SUFFIX,
                    );
                    let value = kept
                        .value(&key)
                        .or_else(|| file.get(&key))
                        .filter(|value| !value.is_empty())?
                        .to_owned();
                    Some(Filing {
                        name: super::named(&one.plugin, &secret.id),
                        key,
                        value,
                    })
                })
        })
        .collect()
}

fn change(one: &Keeping<'_>, current: Option<String>, filings: &[Filing], stamp: &str) -> Change {
    Change {
        at: stamp.to_owned(),
        operation: super::OPERATION.to_owned(),
        target: one.name.to_owned(),
        kind: Kind::Secret {
            name: one.name.to_owned(),
            from: one.from.as_str().to_owned(),
            previous: None,
            current,
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
    let kept = earlier(ctx);
    let rows = stacked(&kept, &file);
    let key_of = |name: &str| {
        rows.iter()
            .find(|(row, _)| row.name == name)
            .map(|(row, _)| row.key.clone())
    };
    if !rows.is_empty() {
        let journal = env.with_file_name(JOURNAL);
        let at = ctx.seconds();
        let stamp = ctx.stamp();
        let keeping: Vec<Keeping<'_>> = rows
            .iter()
            .filter(|(row, _)| row.secret)
            .map(|(row, value)| Keeping {
                name: &row.name,
                value,
                from: Source::Operator,
                at,
            })
            .collect();
        let random = ctx.seams.random.as_ref();
        super::secrets::filed(&env.with_file_name(SECRETS), &keeping, random, &|absent| {
            let changes: Vec<Change> = absent
                .iter()
                .map(|(one, current)| filed_secret(one, current.clone(), key_of(one.name), &stamp))
                .collect();
            crate::app::recover::journalled(&journal, &changes)
        })?;
        let filing: Vec<super::store::Filing<'_>> = rows
            .iter()
            .filter(|(row, _)| !row.secret)
            .map(|(row, value)| (row.name.as_str(), *value))
            .collect();
        super::store::filed(&settings, &filing, &|absent| {
            let changes: Vec<Change> = absent
                .iter()
                .map(|(name, value)| filed_setting(name, value, key_of(name), &stamp))
                .collect();
            crate::app::recover::journalled(&journal, &changes)
        })?;
        for (row, _) in &rows {
            store::unset(env, &row.key)?;
        }
    }
    nas_folded(ctx, env, &file)?;
    super::store::marked_stack_filed(&settings)
}

/// Each of `kept` that `file` holds a value for, with that value.
fn stacked<'a>(kept: &'a [Earlier], file: &'a EnvFile) -> Vec<(&'a Earlier, &'a str)> {
    kept.iter()
        .filter_map(|row| {
            file.get(&row.key)
                .filter(|value| !value.is_empty())
                .map(|value| (row, value))
        })
        .collect()
}

/// What filing would keep in the stores, read from the environment file and the stores as
/// they stand: each setting and each secret by name, in the order filing keeps them, which
/// a store takes only where it holds nothing under that name yet.
pub(crate) fn plan(ctx: &Ctx) -> Plan {
    let Some(env) = ctx.settings.env_file.as_deref() else {
        return Plan::default();
    };
    let Ok(file) = store::read(env) else {
        return Plan::default();
    };
    let settings = Settings::at(&env.with_file_name(SETTINGS)).unwrap_or_default();
    let mut plan = Plan::default();
    if !settings.is_filed() {
        let installed = crate::app::plugins::read(ctx)
            .map(|register| register.installed().to_vec())
            .unwrap_or_default();
        plan.secrets.extend(
            candidates(env, &file, &installed)
                .into_iter()
                .map(|filing| (filing.name, filing.value, Source::Recipe)),
        );
    }
    if !settings.is_stack_filed() {
        for (row, value) in stacked(&earlier(ctx), &file) {
            let (name, value) = (row.name.clone(), (*value).to_owned());
            if row.secret {
                plan.secrets.push((name, value, Source::Operator));
            } else {
                plan.settings.push((name, value));
            }
        }
    }
    plan.secrets.extend(
        credentials_in(&file)
            .into_iter()
            .map(|(key, name, value)| (name, value.to_owned(), legacy::source_of(&key))),
    );
    plan
}

/// What filing would keep in the stores: each setting as its name and value, and each
/// secret as its name, value and where it came from.
#[derive(Debug, Default)]
pub(crate) struct Plan {
    pub(crate) settings: Vec<(String, String)>,
    pub(crate) secrets: Vec<(String, String, Source)>,
}

/// Every variable an earlier release kept a setting's or a secret's value under, one
/// setting or secret each: what the stack's services and every first-party plugin
/// installed name in `was`, then each the core's table names that none of them does.
fn earlier(ctx: &Ctx) -> Vec<Earlier> {
    let stack = ctx
        .stack
        .checked_manifest(ctx.today())
        .map(|manifest| {
            manifest
                .services
                .iter()
                .flat_map(|service| earlier::of([&service.env], &service.id, &service.secret))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let installed = crate::app::plugins::read(ctx)
        .map(|register| {
            register
                .installed()
                .iter()
                .filter(|one| holds(EMBEDDED, one))
                .flat_map(crate::plugin::earlier::of)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let declared = earlier::distinct(stack.into_iter().chain(installed));
    let table = legacy::TABLE
        .iter()
        .filter(|row| {
            !declared
                .iter()
                .any(|one| one.key == row.key || one.name == row.name)
        })
        .map(|row| Earlier {
            key: row.key.to_owned(),
            name: row.name.to_owned(),
            secret: row.secret,
        })
        .collect::<Vec<_>>();
    let mut seen = std::collections::BTreeSet::new();
    declared
        .into_iter()
        .chain(table)
        .filter(|one| seen.insert(one.key.clone()))
        .collect()
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
    crate::app::recover::journalled(&env.with_file_name(JOURNAL), &changes)?;
    store::set(env, DATA_ROOT_KEY, &folded)?;
    store::set(env, NAS_HOST_KEY, "")?;
    store::set(env, NAS_EXPORT_KEY, "")
}

fn filed_secret(
    one: &Keeping<'_>,
    current: Option<String>,
    key: Option<String>,
    stamp: &str,
) -> Change {
    Change {
        at: stamp.to_owned(),
        operation: super::OPERATION.to_owned(),
        target: one.name.to_owned(),
        kind: Kind::Secret {
            name: one.name.to_owned(),
            from: one.from.as_str().to_owned(),
            previous: None,
            current,
            filed: key,
        },
    }
}

fn filed_setting(name: &str, value: &str, key: Option<String>, stamp: &str) -> Change {
    Change {
        at: stamp.to_owned(),
        operation: super::OPERATION.to_owned(),
        target: name.to_owned(),
        kind: Kind::Setting {
            name: name.to_owned(),
            previous: None,
            current: Some(value.to_owned()),
            filed: key,
        },
    }
}
