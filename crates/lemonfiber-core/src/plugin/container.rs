//! The Compose entry lemonfiber writes for each of a plugin's services, from the one
//! typed service model its install record keeps, and nothing else.
//!
//! A function of the record and of what [`Reading`] answers: the operator's settings,
//! the addresses asks reach, the egress guard a service runs inside and the host's
//! render group. Every value a manifest supplied is written so Compose substitutes
//! nothing in it; the variables this writes itself are the only substitutions an entry
//! carries.

use std::collections::BTreeMap;
use std::path::PathBuf;

use lemonfiber_manifest::container::{
    is_source, is_target, lies_inside, Constant, Mount, Posture, Shape, UrlOf, UserMode, Value,
    CONFIGURATION,
};
use serde::Serialize;

use super::installed::{Installed, Placed, Reached};
use crate::config::{DEFAULT_ZONE, PGID_KEY, PUID_KEY, ZONE_KEY};

/// The word in front of a plugin's id, which is the profile its services sit in.
const PROFILE: &str = "plugin-";

/// The Compose profile this plugin's services sit in.
#[must_use]
pub fn profile(plugin: &str) -> String {
    format!("{PROFILE}{plugin}")
}

/// The library, mounted exactly as every bundled service mounts it.
const LIBRARY: &str = "${DATA_ROOT:-./data}:/data";

/// The uid and gid Compose writes while the stack's environment file names neither.
const OPERATOR_FALLBACK: &str = "1000";

/// What brings a container back after it stops for any reason but being taken down.
const RESTART: &str = "unless-stopped";

/// The interface a household port is published on.
const HOUSEHOLD: &str = "${LAN_BIND:-0.0.0.0}";

/// The interface an operator surface or an adapter's contracts are published on.
pub(super) const LOOPBACK: &str = "127.0.0.1";

/// What the container may not do once it has started: gain a privilege.
const SECURITY: [&str; 1] = ["no-new-privileges:true"];

/// Every kernel capability, all of which are dropped before any is given back.
const DROPPED: [&str; 1] = ["ALL"];

/// The capabilities a standard container is given back: what an image that starts as
/// root needs to give its files to the operator, become them and signal its processes.
const KEPT: [&str; 6] = [
    "CHOWN",
    "DAC_OVERRIDE",
    "FOWNER",
    "SETGID",
    "SETUID",
    "KILL",
];

/// How many processes a container may hold at once.
const PROCESSES: u32 = 1024;

/// The network a service whose egress guard nothing fills is held to: none at all.
const NO_NETWORK: &str = "none";

/// The condition a service running inside its guard's network waits for.
const HEALTHY: &str = "service_healthy";

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
}

impl Reading<'_> {
    /// Nothing recorded, nothing filled, no guard and no render devices.
    #[must_use]
    pub const fn bare() -> Reading<'static> {
        Reading {
            setting: &|_| None,
            secrets_file: &|_| None,
            crossing: &|_, _| None,
            address: &|_, _| None,
            guard: &|_| None,
            render_group: None,
        }
    }
}

/// The Compose document lemonfiber writes for an installed plugin.
#[must_use]
pub fn written(installed: &Installed, reading: &Reading<'_>) -> String {
    let document = Document {
        services: Services(
            installed
                .services
                .iter()
                .map(|placed| {
                    (
                        placed.service.as_str(),
                        entry(&installed.plugin, placed, reading),
                    )
                })
                .collect(),
        ),
    };
    serde_norway::to_string(&document).unwrap_or_default()
}

/// The document: services, and nothing else at the top.
#[derive(Serialize)]
struct Document<'a> {
    /// Every one of the plugin's services, by name.
    services: Services<'a>,
}

/// Services in the order the record keeps them.
struct Services<'a>(Vec<(&'a str, Entry)>);

impl Serialize for Services<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.iter().map(|(name, entry)| (name, entry)))
    }
}

/// One service's entry, and the whole of what one can carry.
#[derive(Serialize)]
struct Entry {
    /// The registry path joined to the digest that pins it.
    image: String,
    /// The plugin's own profile.
    profiles: Vec<String>,
    /// What brings it back.
    restart: &'static str,
    /// Who it runs as, where not the image's choice.
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
    /// Its environment.
    environment: BTreeMap<String, String>,
    /// Its own secrets file, where it names a secret.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    env_file: Vec<String>,
    /// Where it is published.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    ports: Vec<String>,
    /// The library where it asked for it, its own directory and what lies inside it.
    volumes: Vec<String>,
    /// Its scratch paths in memory.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tmpfs: Vec<String>,
    /// The networks it joins.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    networks: Vec<String>,
    /// The network it runs inside instead of its own.
    #[serde(skip_serializing_if = "Option::is_none")]
    network_mode: Option<String>,
    /// The guard it waits on, where it runs inside one.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    depends_on: BTreeMap<String, Waiting>,
    /// What it may not gain once started.
    security_opt: [&'static str; 1],
    /// The capabilities it starts without.
    cap_drop: [&'static str; 1],
    /// The capabilities it is given back, and those its shape adds.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cap_add: Vec<&'static str>,
    /// The devices its shape adds.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    devices: Vec<String>,
    /// The groups its shape adds.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    group_add: Vec<String>,
    /// How many processes it may hold.
    pids_limit: u32,
    /// Whether its root filesystem is read-only.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    read_only: bool,
    /// Its memory ceiling.
    #[serde(skip_serializing_if = "Option::is_none")]
    mem_limit: Option<String>,
}

/// What a service waits for in the one it depends on.
#[derive(Serialize)]
struct Waiting {
    /// The state it waits for.
    condition: &'static str,
}

/// One service's entry.
fn entry(plugin: &str, placed: &Placed, reading: &Reading<'_>) -> Entry {
    let fields = &placed.fields;
    let confined = fields.posture == Posture::Confined;
    let guard = fields
        .network
        .as_ref()
        .map(|network| (reading.guard)(&network.via));
    let (render_devices, render_groups) = rendered(placed.shape, reading.render_group);
    Entry {
        image: format!("{}@{}", literal(&placed.image), literal(&placed.digest)),
        profiles: vec![profile(plugin)],
        restart: RESTART,
        user: (user_of(placed) == UserMode::Uid).then(|| {
            format!(
                "{}:{}",
                variable(PUID_KEY, OPERATOR_FALLBACK),
                variable(PGID_KEY, OPERATOR_FALLBACK)
            )
        }),
        environment: environment(placed, reading),
        env_file: secrets_file(placed, reading)
            .map(|path| literal(&path.display().to_string()))
            .into_iter()
            .collect(),
        ports: if guard.is_some() {
            Vec::new()
        } else {
            ports(placed)
        },
        volumes: volumes(placed),
        tmpfs: tmpfs(placed).map(|path| literal(path)).collect(),
        networks: if guard.is_some() {
            Vec::new()
        } else {
            placed.networks.iter().map(|name| literal(name)).collect()
        },
        network_mode: guard.as_ref().map(|held| match held {
            Some(service) => format!("service:{}", literal(service)),
            None => NO_NETWORK.to_owned(),
        }),
        depends_on: guard
            .flatten()
            .map(|service| (literal(&service), Waiting { condition: HEALTHY }))
            .into_iter()
            .collect(),
        security_opt: SECURITY,
        cap_drop: DROPPED,
        cap_add: if confined {
            Vec::new()
        } else {
            KEPT.into_iter()
                .chain(
                    placed
                        .shape
                        .iter()
                        .flat_map(|shape| shape.grants().iter().copied()),
                )
                .collect()
        },
        devices: placed
            .shape
            .filter(|shape| *shape != Shape::Render)
            .iter()
            .flat_map(|shape| shape.devices().iter())
            .map(|device| format!("{device}:{device}"))
            .chain(render_devices)
            .collect(),
        group_add: render_groups,
        pids_limit: PROCESSES,
        read_only: confined,
        mem_limit: fields.mem_limit_mib.map(|mib| format!("{mib}m")),
    }
}

/// Who the service runs as, the declared mode or the one its kind requires.
const fn user_of(placed: &Placed) -> UserMode {
    UserMode::of(
        placed.fields.user_mode,
        !placed.speaks.is_empty(),
        placed.fields.posture,
    )
}

/// The render devices and group, where the shape asks for them and the host has them.
fn rendered(shape: Option<Shape>, group: Option<u32>) -> (Vec<String>, Vec<String>) {
    match (shape, group) {
        (Some(Shape::Render), Some(group)) => (
            Shape::Render
                .devices()
                .iter()
                .map(|device| format!("{device}:{device}"))
                .collect(),
            vec![group.to_string()],
        ),
        _ => (Vec::new(), Vec::new()),
    }
}

/// Its environment: every variable the manifest declares that has a value to write, then
/// the time zone and the operator's pair where it asks for it, which no record overrides.
fn environment(placed: &Placed, reading: &Reading<'_>) -> BTreeMap<String, String> {
    let mut written: BTreeMap<String, String> = placed
        .fields
        .env
        .iter()
        .filter_map(|(name, value)| {
            let text = valued(&placed.service, value, reading)?;
            Some((name.clone(), literal(&text)))
        })
        .collect();
    written.insert(ZONE_KEY.to_owned(), variable(ZONE_KEY, DEFAULT_ZONE));
    if user_of(placed) == UserMode::PuidEnv {
        for key in [PUID_KEY, PGID_KEY] {
            written.insert(key.to_owned(), variable(key, OPERATOR_FALLBACK));
        }
    }
    written
}

/// A variable of the stack's environment file, with what Compose writes while it names
/// none.
fn variable(key: &str, fallback: &str) -> String {
    format!("${{{key}:-{fallback}}}")
}

/// What one variable is written as, or nothing where it has no value to write.
fn valued(service: &str, value: &Value, reading: &Reading<'_>) -> Option<String> {
    match value {
        Value::Constant(constant) => Some(constant_text(constant)),
        Value::Setting(setting) => Some(
            (reading.setting)(&setting.setting).unwrap_or_else(|| constant_text(&setting.default)),
        ),
        Value::Url(url) => (reading.address)(service, url),
        Value::Secret(_) => None,
    }
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
fn holding(installed: &Installed, id: &str, reading: &Reading<'_>) -> Holding {
    installed
        .declared
        .secrets
        .iter()
        .find(|secret| secret.id == id)
        .and_then(|secret| (reading.crossing)(&installed.plugin, &secret.of))
        .unwrap_or_else(|| Holding::Store(crate::settings::named(&installed.plugin, id)))
}

/// A constant as the text an environment variable holds.
fn constant_text(constant: &Constant) -> String {
    match constant {
        Constant::Text(text) => text.clone(),
        Constant::Number(number) => number.to_string(),
        Constant::Flag(flag) => flag.to_string(),
    }
}

/// Every port it publishes, each on its tier, and an adapter's contract port on the
/// loopback interface at a port the engine chooses.
fn ports(placed: &Placed) -> Vec<String> {
    let Some(reached) = &placed.reached else {
        return contract_port(placed).into_iter().collect();
    };
    let on = published(reached);
    let first = reached.port();
    std::iter::once((first, placed.listens.unwrap_or(first)))
        .chain(
            placed
                .fields
                .publish
                .iter()
                .map(|further| (further.port, further.listens.unwrap_or(further.port))),
        )
        .map(|(host, inside)| format!("{on}:{host}:{inside}"))
        .chain(contract_port(placed))
        .collect()
}

/// An adapter's contract port, on the loopback interface at a port the engine chooses.
fn contract_port(placed: &Placed) -> Option<String> {
    placed
        .listens
        .filter(|_| !placed.speaks.is_empty())
        .map(|listens| format!("{LOOPBACK}::{listens}"))
}

/// The library where it asked for it, its own configuration directory, and each path
/// mounted from its own mount tree.
fn volumes(placed: &Placed) -> Vec<String> {
    let service = literal(&placed.service);
    let mut volumes = Vec::new();
    if placed.takes_data {
        volumes.push(LIBRARY.to_owned());
    }
    volumes.push(mounted(
        &format!("./{}/{service}", super::placing::CONFIGURATION),
        configuration(placed),
        placed.fields.config_ro,
    ));
    volumes.extend(mounts(placed).map(|Mount { from, to, ro }| {
        mounted(
            &format!("./{}/{service}/{}", super::placing::MOUNTS, literal(from)),
            to,
            *ro,
        )
    }));
    volumes
}

/// Where its configuration directory lands: where the record says, where that is a target
/// the model allows, and the model's own default otherwise.
fn configuration(placed: &Placed) -> &str {
    if is_target(&placed.config_path, true) {
        &placed.config_path
    } else {
        CONFIGURATION
    }
}

/// Each further mount the model allows: a source inside its own mount tree and inside no
/// other of its sources, landing where a mount may land.
pub(super) fn mounts(placed: &Placed) -> impl Iterator<Item = &Mount> {
    let all = &placed.fields.mount;
    all.iter().filter(move |mount| {
        is_source(&mount.from)
            && is_target(&mount.to, placed.takes_data)
            && !all
                .iter()
                .any(|other| lies_inside(&mount.from, &other.from))
    })
}

/// Each scratch path in memory the model allows.
fn tmpfs(placed: &Placed) -> impl Iterator<Item = &String> {
    placed
        .fields
        .tmpfs
        .iter()
        .filter(|path| is_target(path, placed.takes_data))
}

/// Whether every path the record keeps for the service is one the model allows.
pub(super) fn sound(placed: &Placed) -> bool {
    is_target(&placed.config_path, true)
        && mounts(placed).count() == placed.fields.mount.len()
        && tmpfs(placed).count() == placed.fields.tmpfs.len()
}

/// One bind mount of `source` at `to`.
fn mounted(source: &str, to: &str, ro: bool) -> String {
    let mode = if ro { ":ro" } else { "" };
    format!("{source}:{}{mode}", literal(to))
}

/// A value Compose reads back as exactly itself.
fn literal(text: &str) -> String {
    text.replace('$', "$$")
}

/// Which interface a tier publishes on.
const fn published(reached: &Reached) -> &'static str {
    match reached {
        Reached::Loopback { .. } => LOOPBACK,
        Reached::Household { .. } => HOUSEHOLD,
    }
}

#[cfg(test)]
mod tests;
