//! The Compose entry lemonfiber writes for each of a plugin's services, from the one
//! typed service model its install record keeps, and nothing else.
//!
//! A function of the record and of what [`Reading`] answers: the operator's settings,
//! the addresses asks reach, the egress guard a service runs inside and the host's
//! render group. Every value a manifest supplied is written so Compose substitutes
//! nothing in it; the variables this writes itself are the only substitutions an entry
//! carries.

use std::path::PathBuf;

use lemonfiber_manifest::container::{is_target, Mount, UrlOf, UserMode, Value};
use serde::Serialize;

use super::installed::{Installed, Placed, Reached};
use crate::config::storage::Export;
use crate::project::entry::{self, literal, Entry, Tier, Unit, Values};
use crate::topology;

/// The word in front of a plugin's id, which is the profile its services sit in.
const PROFILE: &str = "plugin-";

/// The Compose profile this plugin's services sit in.
#[must_use]
pub fn profile(plugin: &str) -> String {
    format!("{PROFILE}{plugin}")
}

/// The interface an operator surface or an adapter's contracts are published on.
pub(super) const LOOPBACK: &str = crate::project::entry::LOOPBACK;

/// One variable of a service's secrets file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    /// The variable's name.
    pub name: String,
    /// Where its value is read from.
    pub holding: Holding,
}

/// Where a secret's value is read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Holding {
    /// The secret store, under this name.
    Store(String),
    /// The environment file, under the setting the credential's holder keeps it in.
    Crossing(String),
    /// Nowhere: the credential may not cross to this service.
    Withheld,
}

/// What an entry's values are read from beyond the record.
pub struct Reading<'a> {
    /// The operator's value for a setting, where one is recorded.
    pub setting: &'a dyn Fn(&str) -> Option<String>,
    /// The named service's secrets file, where there is somewhere to keep one.
    pub secrets_file: &'a dyn Fn(&str) -> Option<PathBuf>,
    /// Where a credential the named service holds is read from for the named plugin,
    /// where the service is not the plugin's own.
    pub crossing: &'a dyn Fn(&str, &str) -> Option<Holding>,
    /// The address the named service's ask reaches, where something fills it.
    pub address: &'a dyn Fn(&str, &UrlOf) -> Option<String>,
    /// The service that holds the tunnel of the capability a service runs inside.
    pub guard: &'a dyn Fn(&str) -> Option<String>,
    /// The group that owns the host's render devices, where the host has them.
    pub render_group: Option<u32>,
    /// The NFS export the library is kept on, where it is kept on one.
    pub nfs: Option<&'a Export>,
}

impl Reading<'_> {
    /// Nothing recorded, nothing filled, no guard, no render devices and the library on
    /// this machine.
    #[must_use]
    pub const fn bare() -> Reading<'static> {
        Reading {
            setting: &|_| None,
            secrets_file: &|_| None,
            crossing: &|_, _| None,
            address: &|_, _| None,
            guard: &|_| None,
            render_group: None,
            nfs: None,
        }
    }
}

/// The Compose document lemonfiber writes for an installed plugin.
#[must_use]
pub fn written(installed: &Installed, reading: &Reading<'_>) -> String {
    let setting = |_: &str, key: &str| (reading.setting)(key);
    let values = Values {
        setting: &setting,
        address: reading.address,
        render_group: reading.render_group,
        nfs: reading.nfs,
    };
    let taken = installed.services.iter().any(|placed| placed.takes_data);
    let document = Document {
        services: Services(
            installed
                .services
                .iter()
                .map(|placed| {
                    (
                        placed.service.as_str(),
                        entry::entry(
                            &unit(&installed.plugin, placed, reading),
                            &standing(placed, reading),
                            &values,
                        ),
                    )
                })
                .collect(),
        ),
        volumes: crate::project::document::library(reading.nfs, taken),
    };
    crate::project::document::yaml(&document)
}

/// The document: services, and the library's volume where it is kept on an NFS export.
#[derive(Serialize)]
struct Document<'a> {
    /// Every one of the plugin's services, by name.
    services: Services<'a>,
    /// The library's volume, where it is kept on an NFS export and a service takes it.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    volumes: std::collections::BTreeMap<&'static str, crate::project::document::Volume>,
}

/// Services in the order the record keeps them.
struct Services<'a>(Vec<(&'a str, Entry)>);

impl Serialize for Services<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.iter().map(|(name, entry)| (name, entry)))
    }
}

/// One of a plugin's services as its entry needs it.
pub(crate) fn unit<'a>(plugin: &str, placed: &'a Placed, reading: &Reading<'_>) -> Unit<'a> {
    let fields = &placed.fields;
    Unit {
        id: &placed.service,
        namespace: &placed.service,
        profile: profile(plugin),
        image: format!("{}@{}", literal(&placed.image), literal(&placed.digest)),
        published: published(placed),
        contract: placed.listens.filter(|_| !placed.speaks.is_empty()),
        takes_data: placed.takes_data,
        config_path: &placed.config_path,
        config_ro: fields.config_ro,
        mounts: &fields.mount,
        tmpfs: &fields.tmpfs,
        env: &fields.env,
        user: user_of(placed),
        posture: fields.posture,
        mem_limit_mib: fields.mem_limit_mib,
        shape: placed.shape,
        secrets_file: secrets_file(placed, reading),
    }
}

/// Where a plugin's service stands in its own document: inside its guard where it takes
/// one, on no network where nothing fills that guard, and on the stack's networks it was
/// installed beside otherwise.
fn standing(placed: &Placed, reading: &Reading<'_>) -> topology::Placed {
    match placed
        .fields
        .network
        .as_ref()
        .map(|network| (reading.guard)(&network.via))
    {
        Some(Some(service)) => topology::Placed::Inside(service),
        Some(None) => topology::Placed::Nowhere,
        None => topology::Placed::On(
            placed
                .networks
                .iter()
                .map(|name| (name.clone(), None))
                .collect(),
        ),
    }
}

/// Every port it publishes, each on its tier.
fn published(placed: &Placed) -> Vec<(Tier, u16, u16)> {
    let Some(reached) = &placed.reached else {
        return Vec::new();
    };
    let tier = match reached {
        Reached::Loopback { .. } => Tier::Loopback,
        Reached::Household { .. } => Tier::Household,
    };
    let first = reached.port();
    std::iter::once((first, placed.listens.unwrap_or(first)))
        .chain(
            placed
                .fields
                .publish
                .iter()
                .map(|further| (further.port, further.listens.unwrap_or(further.port))),
        )
        .map(|(host, inside)| (tier, host, inside))
        .collect()
}

/// Who the service runs as, the declared mode or the one its kind requires.
pub(crate) const fn user_of(placed: &Placed) -> UserMode {
    UserMode::of(
        placed.fields.user_mode,
        !placed.speaks.is_empty(),
        placed.fields.posture,
    )
}

/// The service's secrets file, where it names a secret and there is somewhere to keep one.
pub(crate) fn secrets_file(placed: &Placed, reading: &Reading<'_>) -> Option<PathBuf> {
    placed
        .fields
        .env
        .values()
        .any(|value| matches!(value, Value::Secret(_)))
        .then(|| (reading.secrets_file)(&placed.service))
        .flatten()
}

/// Every variable of the service's secrets file, each with where its value is read from.
pub(crate) fn variables(
    installed: &Installed,
    placed: &Placed,
    reading: &Reading<'_>,
) -> Vec<Variable> {
    placed
        .fields
        .env
        .iter()
        .filter_map(|(name, value)| match value {
            Value::Secret(secret) => Some(Variable {
                name: name.clone(),
                holding: holding(installed, &secret.secret, reading),
            }),
            _ => None,
        })
        .collect()
}

/// Where the plugin's secret `id` is read from: its holder where it is a credential that
/// crosses, the secret store otherwise.
pub(crate) fn holding(installed: &Installed, id: &str, reading: &Reading<'_>) -> Holding {
    installed
        .declared
        .secrets
        .iter()
        .find(|secret| secret.id == id)
        .and_then(|secret| (reading.crossing)(&installed.plugin, &secret.of))
        .unwrap_or_else(|| Holding::Store(crate::settings::named(&installed.plugin, id)))
}

/// Each further mount the model allows: a source inside its own mount tree and inside no
/// other of its sources, landing where a mount may land.
pub(super) fn mounts(placed: &Placed) -> impl Iterator<Item = &Mount> {
    entry::mounts(&placed.fields.mount, placed.takes_data)
}

/// Whether every path the record keeps for the service is one the model allows.
pub(super) fn sound(placed: &Placed) -> bool {
    is_target(&placed.config_path, true)
        && mounts(placed).count() == placed.fields.mount.len()
        && placed
            .fields
            .tmpfs
            .iter()
            .all(|path| is_target(path, placed.takes_data))
}

#[cfg(test)]
mod tests;
