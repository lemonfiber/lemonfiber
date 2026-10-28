use std::collections::BTreeMap;

use lemonfiber_manifest::container::{Value, NAMESPACED};
use lemonfiber_manifest::Manifest;

use crate::model::{Owner, Reader};
use crate::plugin::Installed;

#[derive(Debug, Default)]
pub(crate) struct Readers {
    pub(crate) settings: BTreeMap<String, Vec<Reader>>,
    pub(crate) secrets: BTreeMap<String, Vec<Reader>>,
}

impl Readers {
    fn add(&mut self, service: &str, namespace: &str, env: &BTreeMap<String, Value>) {
        for (variable, value) in env {
            let (into, name, default) = match value {
                Value::Setting(setting) => (
                    &mut self.settings,
                    setting.setting.clone(),
                    Some(setting.default.text()),
                ),
                Value::Secret(secret) => (
                    &mut self.secrets,
                    super::named(namespace, &secret.secret),
                    None,
                ),
                Value::Constant(_) | Value::Url(_) => continue,
            };
            into.entry(name).or_default().push(Reader {
                service: service.to_owned(),
                variable: variable.clone(),
                default,
            });
        }
    }
}

pub(crate) fn readers(manifest: &Manifest, installed: &[Installed]) -> Readers {
    let mut readers = Readers::default();
    for service in &manifest.services {
        let container = service.container();
        readers.add(container.id, container.namespace, container.env);
    }
    for one in installed {
        for placed in &one.services {
            readers.add(&placed.service, &one.plugin, &placed.fields.env);
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
