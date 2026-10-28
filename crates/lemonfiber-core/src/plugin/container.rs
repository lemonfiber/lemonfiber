//! One of a plugin's services as the generated project writes it, from the one typed
//! service model its install record keeps, and the secrets file its entry names.

use std::path::PathBuf;

use lemonfiber_manifest::container::{exposes, inside, is_target, Mount, Reach, UserMode, Value};

use super::installed::{Installed, Placed, Reached};
use crate::project::entry::{self, literal, Tier, Unit};

/// The word in front of a plugin's id, which is the profile its services sit in.
const PROFILE: &str = "plugin-";

/// The Compose profile a plugin's services sit in where the bundle places them in none of
/// the stack's.
#[must_use]
pub fn profile(plugin: &str) -> String {
    format!("{PROFILE}{plugin}")
}

/// Whether `profile` is the one a plugin's services sit in.
pub(crate) fn is_a_plugins(profile: &str) -> bool {
    profile.starts_with(PROFILE)
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

/// Where a plugin's secrets are read from.
pub struct Reading<'a> {
    /// The named service's secrets file, where there is somewhere to keep one.
    pub secrets_file: &'a dyn Fn(&str) -> Option<PathBuf>,
    /// Where a credential the named service holds is read from for the named plugin,
    /// where the service is not the plugin's own.
    pub crossing: &'a dyn Fn(&str, &str) -> Option<Holding>,
}

impl Reading<'_> {
    /// No secrets file and nothing crossing.
    #[must_use]
    pub const fn bare() -> Reading<'static> {
        Reading {
            secrets_file: &|_| None,
            crossing: &|_, _| None,
        }
    }
}

/// One of `installed`'s services as its entry needs it, in the profile its services sit in.
pub(crate) fn unit<'a>(
    installed: &Installed,
    placed: &'a Placed,
    reading: &Reading<'_>,
) -> Unit<'a> {
    let fields = &placed.fields;
    Unit {
        id: &placed.service,
        namespace: &placed.service,
        profile: super::profile_of(installed),
        image: format!("{}@{}", literal(&placed.image), literal(&placed.digest)),
        published: published(placed),
        contract: placed.contract_port(),
        takes_data: placed.takes_data,
        config_path: &placed.config_path,
        config_ro: fields.config_ro,
        mounts: &fields.mount,
        shares: placed
            .fronts
            .as_deref()
            .zip(fields.shares_config.as_deref()),
        tmpfs: &fields.tmpfs,
        env: &fields.env,
        user: user_of(placed),
        posture: fields.posture,
        mem_limit_mib: fields.mem_limit_mib,
        shape: placed.shape,
        secrets_file: secrets_file(placed, reading),
        command: Vec::new(),
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
    let contract = placed.contract_port();
    std::iter::once((Reach::Port, first, inside(first, placed.listens)))
        .chain(placed.fields.publish.iter().map(|further| {
            (
                Reach::Publish,
                further.port,
                inside(further.port, further.listens),
            )
        }))
        .filter(|(how, _, at)| !exposes(*how, *at, contract, placed.listens))
        .map(|(_, host, at)| (tier, host, at))
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
    crate::project::names_a_secret(&placed.fields.env)
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

/// Where the plugin's secret `id` is read from: the secret store where it is the plugin's
/// own, and its holder where it is a credential that crosses, withheld where nothing may
/// hand it over.
pub(crate) fn holding(installed: &Installed, id: &str, reading: &Reading<'_>) -> Holding {
    match installed
        .declared
        .secrets
        .iter()
        .find(|secret| secret.id == id)
    {
        Some(secret) if !own_secret(&installed.plugin, services_of(installed), &secret.of) => {
            (reading.crossing)(&installed.plugin, &secret.of).unwrap_or(Holding::Withheld)
        }
        _ => Holding::Store(crate::settings::named(&installed.plugin, id)),
    }
}

/// Whether a secret whose `of` is `of` is the plugin `plugin`'s own: of the plugin or one
/// of its `services`, rather than a credential that crosses to it.
pub(crate) fn own_secret<'a>(
    plugin: &str,
    mut services: impl Iterator<Item = &'a str>,
    of: &str,
) -> bool {
    plugin == of || services.any(|service| service == of)
}

/// The ids of an installed plugin's services.
pub(crate) fn services_of(installed: &Installed) -> impl Iterator<Item = &str> {
    installed
        .services
        .iter()
        .map(|placed| placed.service.as_str())
}

/// Each further mount the model allows: a source inside its own mount tree and inside no
/// other of its sources, landing where a mount may land.
pub(super) fn mounts(placed: &Placed) -> impl Iterator<Item = &Mount> {
    entry::mounts(&placed.fields.mount, placed.takes_data)
}

/// Each scratch path in memory the model allows.
pub(super) fn tmpfs(placed: &Placed) -> impl Iterator<Item = &String> {
    entry::tmpfs(&placed.fields.tmpfs, placed.takes_data)
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
