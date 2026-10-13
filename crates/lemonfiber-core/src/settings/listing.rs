use std::collections::BTreeSet;

use lemonfiber_manifest::container::NAMESPACED;
use lemonfiber_manifest::Manifest;

use crate::app::Ctx;
use crate::model::{SecretFrom, SecretRead, SettingChange, SettingRead, SettingsReport};
use crate::plugin::{Holding, Installed};

use super::readers::{self, Readers};
use super::recorded::Recorded;
use super::{legacy, own, Resolved, Secrets, Source};

pub(super) fn report(
    ctx: &Ctx,
    manifest: &Manifest,
    installed: &[Installed],
    change: Option<SettingChange>,
) -> SettingsReport {
    let readers = readers::readers(manifest, installed);
    let resolved = Resolved::of(ctx, Some(manifest), installed);
    SettingsReport {
        settings: settings(&Recorded::of(ctx), &readers),
        secrets: secrets(ctx, &readers, installed, &resolved),
        unvetted: change.as_ref().and(crate::overlay::unvetted(ctx)),
        change,
        rehearsed: ctx.dry_run,
    }
}

fn settings(recorded: &Recorded, readers: &Readers) -> Vec<SettingRead> {
    let mut names: BTreeSet<&str> = readers.settings.keys().map(String::as_str).collect();
    names.extend(own::CORE);
    names
        .into_iter()
        .map(|name| SettingRead {
            name: name.to_owned(),
            whose: readers::whose(name),
            readers: readers.settings.get(name).cloned().unwrap_or_default(),
            value: recorded.value(name).map(str::to_owned),
            environment: legacy::setting_key(name).map(str::to_owned),
        })
        .collect()
}

pub(super) fn secrets(
    ctx: &Ctx,
    readers: &Readers,
    installed: &[Installed],
    resolved: &Resolved,
) -> Vec<SecretRead> {
    let kept = super::secrets_at(ctx)
        .and_then(|at| Secrets::at(&at).ok())
        .unwrap_or_default();
    readers
        .secrets
        .iter()
        .map(|(name, reading)| {
            let (from, held) = match holding(name, installed, resolved) {
                Holding::Store(name) => (
                    kept.source(&name).map(|source| match source {
                        Source::Operator => SecretFrom::Operator,
                        Source::Recipe => SecretFrom::Recipe,
                    }),
                    kept.value(&name).is_some(),
                ),
                Holding::Crossing(setting) => (
                    Some(SecretFrom::Granted),
                    crate::app::targets::recorded_secret(ctx, &setting).is_some(),
                ),
                Holding::Withheld => (Some(SecretFrom::Granted), false),
            };
            SecretRead {
                name: name.clone(),
                whose: readers::whose(name),
                readers: reading.clone(),
                from,
                held,
            }
        })
        .collect()
}

fn holding(name: &str, installed: &[Installed], resolved: &Resolved) -> Holding {
    name.split_once(NAMESPACED)
        .and_then(|(namespace, id)| {
            let one = installed.iter().find(|one| one.plugin == namespace)?;
            Some(resolved.reading(&|_, _| None, |reading| {
                crate::plugin::holding(one, id, reading)
            }))
        })
        .unwrap_or_else(|| Holding::Store(name.to_owned()))
}
