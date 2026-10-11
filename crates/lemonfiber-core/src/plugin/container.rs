//! The Compose entry lemonfiber writes for each of a plugin's services, from the one
//! typed service model its install record keeps, and nothing else.
//!
//! A function of the record and of what [`Reading`] answers: the operator's settings,
//! the addresses asks reach, the egress guard a service runs inside and the host's
//! render group. Every value a manifest supplied is written so Compose substitutes
//! nothing in it; the variables this writes itself are the only substitutions an entry
//! carries.

use std::collections::BTreeMap;

use lemonfiber_manifest::container::{Constant, Mount, Posture, Shape, UrlOf, UserMode, Value};
use serde::Serialize;

use super::installed::{Installed, Placed, Reached};

/// The word in front of a plugin's id, which is the profile its services sit in.
const PROFILE: &str = "plugin-";

/// The Compose profile this plugin's services sit in.
#[must_use]
pub fn profile(plugin: &str) -> String {
    format!("{PROFILE}{plugin}")
}

/// Where lemonfiber keeps a service's own configuration directory.
const CONFIGURATION: &str = "./config";

/// The library, mounted exactly as every bundled service mounts it.
const LIBRARY: &str = "${DATA_ROOT:-./data}:/data";

/// The operator's uid and gid, spelled as the stack's own images are.
const OPERATOR: &str = "${PUID:-1000}:${PGID:-1000}";

/// The operator's uid alone.
const OPERATOR_UID: &str = "${PUID:-1000}";

/// The operator's gid alone.
const OPERATOR_GID: &str = "${PGID:-1000}";

/// The operator's time zone, written into every entry.
const TIME_ZONE: &str = "${TZ:-Europe/Amsterdam}";

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

/// What an entry's values are read from beyond the record.
pub struct Reading<'a> {
    /// The operator's value for a setting, where one is recorded.
    pub setting: &'a dyn Fn(&str) -> Option<String>,
    /// The address an ask reaches, where something fills it.
    pub address: &'a dyn Fn(&UrlOf) -> Option<String>,
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
            address: &|_| None,
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
    serde_yaml_ng::to_string(&document).unwrap_or_default()
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
    user: Option<&'static str>,
    /// Its environment.
    environment: BTreeMap<String, String>,
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
        user: (user_of(placed) == UserMode::Uid).then_some(OPERATOR),
        environment: environment(placed, reading),
        ports: if guard.is_some() {
            Vec::new()
        } else {
            ports(placed)
        },
        volumes: volumes(placed),
        tmpfs: fields.tmpfs.iter().map(|path| literal(path)).collect(),
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
fn user_of(placed: &Placed) -> UserMode {
    match placed.fields.user_mode {
        Some(mode) => mode,
        None if !placed.speaks.is_empty() || placed.fields.posture == Posture::Confined => {
            UserMode::Uid
        }
        None => UserMode::Image,
    }
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

/// Its environment: the time zone, the operator's pair where it asks for it, and every
/// variable the manifest declares that has a value to write.
fn environment(placed: &Placed, reading: &Reading<'_>) -> BTreeMap<String, String> {
    let mut written = BTreeMap::from([("TZ".to_owned(), TIME_ZONE.to_owned())]);
    if user_of(placed) == UserMode::PuidEnv {
        written.insert("PUID".to_owned(), OPERATOR_UID.to_owned());
        written.insert("PGID".to_owned(), OPERATOR_GID.to_owned());
    }
    for (name, value) in &placed.fields.env {
        if let Some(text) = valued(value, reading) {
            written.insert(name.clone(), literal(&text));
        }
    }
    written
}

/// What one variable is written as, or nothing where it has no value to write.
fn valued(value: &Value, reading: &Reading<'_>) -> Option<String> {
    match value {
        Value::Constant(constant) => Some(constant_text(constant)),
        Value::Setting(setting) => Some(
            (reading.setting)(&setting.setting).unwrap_or_else(|| constant_text(&setting.default)),
        ),
        Value::Url(url) => (reading.address)(url),
        Value::Secret(_) => None,
    }
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
        &format!("{CONFIGURATION}/{service}"),
        &placed.config_path,
        placed.fields.config_ro,
    ));
    volumes.extend(placed.fields.mount.iter().map(|Mount { from, to, ro }| {
        mounted(
            &format!("./{}/{service}/{}", super::placing::MOUNTS, literal(from)),
            to,
            *ro,
        )
    }));
    volumes
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
