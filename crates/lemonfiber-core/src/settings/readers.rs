use std::collections::BTreeMap;

use lemonfiber_manifest::container::choices::{self, Choice};
use lemonfiber_manifest::container::{Constant, Value, NAMESPACED};
use lemonfiber_manifest::Manifest;

use crate::model::{Choosable, Owner, Reader};
use crate::plugin::Installed;

#[derive(Debug, Default)]
pub(crate) struct Readers {
    pub(crate) settings: BTreeMap<String, Vec<Reader>>,
    pub(crate) secrets: BTreeMap<String, Vec<Reader>>,
    pub(crate) choices: BTreeMap<String, Vec<Choosable>>,
}

impl Readers {
    fn add(
        &mut self,
        service: &str,
        namespace: &str,
        env: &BTreeMap<String, Value>,
        recorded: &dyn Fn(&str) -> Option<String>,
    ) {
        let held = |name: &str| recorded(name).or_else(|| choices::defaulted(env, name));
        for (variable, value) in env {
            let (into, name, default, needs) = match value {
                Value::Setting(setting) => {
                    if let Some(one_of) = &setting.one_of {
                        self.choices
                            .entry(setting.setting.clone())
                            .or_insert_with(|| choosable(one_of, &held));
                    }
                    (
                        &mut self.settings,
                        setting.setting.clone(),
                        Some(setting.default.text()),
                        Some(&setting.needs),
                    )
                }
                Value::Secret(secret) => (
                    &mut self.secrets,
                    super::named(namespace, &secret.secret),
                    None,
                    None,
                ),
                Value::Constant(_) | Value::Url(_) => continue,
            };
            into.entry(name).or_default().push(Reader {
                service: service.to_owned(),
                variable: variable.clone(),
                default,
                needs: needs.map(texts).unwrap_or_default(),
                met: needs.is_none_or(|needs| choices::met(needs, &held)),
            });
        }
    }
}

/// Every value `one_of` lists, each with what it needs and whether `held` meets it.
fn choosable(one_of: &[Choice], held: &dyn Fn(&str) -> Option<String>) -> Vec<Choosable> {
    one_of
        .iter()
        .flat_map(|choice| {
            choice.values().iter().map(move |value| Choosable {
                value: value.text(),
                needs: choice.needs().map(texts).unwrap_or_default(),
                open: choice.needs().is_none_or(|needs| choices::met(needs, held)),
            })
        })
        .collect()
}

/// `needs`, each value as the text a variable holds.
fn texts(needs: &choices::Needs) -> BTreeMap<String, Vec<String>> {
    needs
        .iter()
        .map(|(setting, values)| (setting.clone(), values.iter().map(Constant::text).collect()))
        .collect()
}

/// Every setting and secret `manifest`'s services and `installed`'s read, with what each
/// reference needs and every value a setting accepts, as `recorded` says what each
/// setting holds.
pub(crate) fn readers(
    manifest: &Manifest,
    installed: &[Installed],
    recorded: &dyn Fn(&str) -> Option<String>,
) -> Readers {
    let mut readers = Readers::default();
    for service in &manifest.services {
        let container = service.container();
        readers.add(container.id, container.namespace, container.env, recorded);
    }
    for one in installed {
        for placed in &one.services {
            readers.add(&placed.service, &one.plugin, &placed.fields.env, recorded);
        }
    }
    for read in readers
        .settings
        .values_mut()
        .chain(readers.secrets.values_mut())
    {
        read.sort();
    }
    readers
}

pub(crate) fn whose(name: &str) -> Owner {
    name.split_once(NAMESPACED)
        .map_or(Owner::Core, |(namespace, _)| Owner::Namespace {
            namespace: namespace.to_owned(),
        })
}

pub(crate) fn services(readers: &[Reader]) -> Vec<String> {
    let mut services: Vec<String> = readers.iter().map(|one| one.service.clone()).collect();
    services.sort();
    services.dedup();
    services
}
