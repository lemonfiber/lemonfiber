//! One service's Compose entry, from the one container model every service is written
//! from: a bundled stack service and a plugin's service alike.
//!
//! Every value a manifest supplied is written so Compose substitutes nothing in it; the
//! variables written here are the only substitutions an entry carries.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::path::PathBuf;

use lemonfiber_manifest::container::choices;
use lemonfiber_manifest::container::{
    is_source, is_target, lies_inside, Mount, Posture, Shape, UrlOf, UserMode, Value, CONFIGURATION,
};
use serde::ser::SerializeMap;
use serde::Serialize;

use crate::config::storage::Export;
use crate::config::{DATA_ROOT_KEY, DEFAULT_ZONE, LAN_BIND_KEY, PGID_KEY, PUID_KEY, ZONE_KEY};
use crate::topology::Placed;

/// Where the library is kept while the operator's environment file names nowhere.
pub(crate) const LIBRARY: &str = "./data";

/// The volume the library is mounted from where it is kept on an NFS export.
pub(crate) const LIBRARY_VOLUME: &str = "data";

/// Where the library lands inside every container that takes it.
pub(crate) const LIBRARY_TARGET: &str = "/data";

/// The uid and gid written while the operator's environment file names neither.
pub(crate) const OPERATOR_FALLBACK: &str = "1000";

/// What brings a container back after it stops for any reason but being taken down.
const RESTART: &str = "unless-stopped";

/// The interface a household port is published on while the operator's environment file
/// narrows it to none.
pub(crate) const HOUSEHOLD: &str = "0.0.0.0";

/// The interface an operator surface or an adapter's contracts are published on.
pub(crate) const LOOPBACK: &str = "127.0.0.1";

/// What the container may not do once it has started: gain a privilege.
pub(crate) const SECURITY: [&str; 1] = ["no-new-privileges:true"];

/// Every kernel capability, all of which are dropped before any is given back.
pub(crate) const DROPPED: [&str; 1] = ["ALL"];

/// The capabilities a standard container is given back: what an image that starts as
/// root needs to give its files to the operator, become them and signal its processes.
pub(crate) const KEPT: [&str; 6] = [
    "CHOWN",
    "DAC_OVERRIDE",
    "FOWNER",
    "SETGID",
    "SETUID",
    "KILL",
];

/// How many processes a container may hold at once.
pub(crate) const PROCESSES: u32 = 1024;

/// The network a service on no network of its own is held to: none at all.
const NO_NETWORK: &str = "none";

/// The condition a service running inside another's network waits for.
const HEALTHY: &str = "service_healthy";

/// The directory beneath the project holding each service's own configuration, the one
/// every entry mounts `./config/<service>` from.
pub(crate) const CONFIGURATION_TREE: &str = "config";

/// Where each service's own mount tree is kept, beneath the project and apart from its
/// configuration directory.
pub(crate) const MOUNT_TREE: &str = "mounts";

/// Which interface a published port is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// The host's own loopback interface.
    Loopback,
    /// The household's network.
    Household,
}

impl Tier {
    /// The interface written in front of the port, where the household's is `household`.
    pub(crate) const fn interface(self, household: &str) -> &str {
        match self {
            Self::Loopback => LOOPBACK,
            Self::Household => household,
        }
    }
}

/// One service, as its entry needs it, whichever manifest declared it.
pub struct Unit<'a> {
    /// Its id, which is its container's name in the project.
    pub id: &'a str,
    /// The namespace its own settings are read from.
    pub namespace: &'a str,
    /// The Compose profile it sits in.
    pub profile: String,
    /// The image it runs, pinned.
    pub image: String,
    /// Where it is published, and on which interface: the first port and each further one.
    pub published: Vec<(Tier, u16, u16)>,
    /// An adapter's contract port, published on loopback at a port the engine chooses.
    pub contract: Option<u16>,
    /// Whether the library is mounted for it.
    pub takes_data: bool,
    /// Where inside the container its configuration directory lands.
    pub config_path: &'a str,
    /// Whether its configuration directory is read-only.
    pub config_ro: bool,
    /// Its further mounts.
    pub mounts: &'a [Mount],
    /// The service of its own plugin whose whole configuration directory it shares, and
    /// where that lands inside it.
    pub shares: Option<(&'a str, &'a str)>,
    /// Its scratch paths in memory.
    pub tmpfs: &'a [String],
    /// Its environment as the manifest declares it.
    pub env: &'a BTreeMap<String, Value>,
    /// Who it runs as.
    pub user: UserMode,
    /// How tightly it is held.
    pub posture: Posture,
    /// Its memory ceiling, in MiB.
    pub mem_limit_mib: Option<u32>,
    /// Its privileged shape.
    pub shape: Option<Shape>,
    /// Its own secrets file, where it names a secret and has one.
    pub secrets_file: Option<PathBuf>,
    /// The arguments its image's program is run with, where not the image's own.
    pub command: Vec<String>,
}

/// What an entry's values are read from beyond its unit.
pub struct Values<'a> {
    /// The operator's value for a setting of the named namespace, where one is recorded.
    pub setting: &'a dyn Fn(&str, &str) -> Option<String>,
    /// The address the named service's ask reaches, where something fills it.
    pub address: &'a dyn Fn(&str, &UrlOf) -> Option<String>,
    /// The group that owns the host's render devices, where the host has them.
    pub render_group: Option<u32>,
    /// The NFS export the library is kept on, where it is kept on one.
    pub nfs: Option<&'a Export>,
    /// The operator's value for a variable of their environment file, where one is
    /// written.
    pub operator: &'a dyn Fn(&str) -> Option<String>,
}

/// One service's entry, and the whole of what one can carry.
#[derive(Serialize)]
pub struct Entry {
    /// The image, pinned.
    image: String,
    /// Its profile.
    profiles: Vec<String>,
    /// What brings it back.
    restart: &'static str,
    /// Who it runs as, where not the image's choice.
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
    /// The arguments its image's program is run with, where not the image's own.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    command: Vec<String>,
    /// Its environment.
    environment: BTreeMap<String, String>,
    /// Its own secrets file, where it names a secret.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    env_file: Vec<String>,
    /// Where it is published.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) ports: Vec<String>,
    /// The library where it asked for it, its own directory and what lies inside it.
    volumes: Vec<String>,
    /// Its scratch paths in memory.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tmpfs: Vec<String>,
    /// The networks it joins.
    #[serde(skip_serializing_if = "Joined::is_empty")]
    networks: Joined,
    /// The network it runs inside instead of its own.
    #[serde(skip_serializing_if = "Option::is_none")]
    network_mode: Option<String>,
    /// The service it waits on, where it runs inside one.
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

/// The networks a service joins: by name alone where none fixes its address, and each
/// with the address it holds where one does.
struct Joined(BTreeMap<String, Option<Ipv4Addr>>);

impl Joined {
    /// Whether it joins none.
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for Joined {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0.values().all(Option::is_none) {
            return serializer.collect_seq(self.0.keys());
        }
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (name, address) in &self.0 {
            let holds = Holds {
                ipv4_address: *address,
            };
            map.serialize_entry(name, &holds)?;
        }
        map.end()
    }
}

/// One network's settings for a service: the address it holds there, where fixed.
#[derive(Serialize)]
struct Holds {
    /// Its fixed address.
    #[serde(skip_serializing_if = "Option::is_none")]
    ipv4_address: Option<Ipv4Addr>,
}

/// What a service waits for in the one it depends on.
#[derive(Serialize)]
struct Waiting {
    /// The state it waits for.
    condition: &'static str,
}

/// One service's entry, placed as `placed` says.
///
/// A service placed inside another's network publishes nothing of its own: whatever it
/// publishes is written on the entry of the service whose network it shares.
#[must_use]
pub fn entry(unit: &Unit<'_>, placed: &Placed, values: &Values<'_>) -> Entry {
    let confined = unit.posture == Posture::Confined;
    let (render_devices, render_groups) = rendered(unit.shape, values.render_group);
    let (networks, network_mode, depends_on) = match placed {
        Placed::On(on) => (on.clone(), None, BTreeMap::new()),
        Placed::Inside(host) => (
            BTreeMap::new(),
            Some(format!("service:{}", literal(host))),
            std::iter::once((literal(host), Waiting { condition: HEALTHY })).collect(),
        ),
        Placed::Nowhere => (
            BTreeMap::new(),
            Some(NO_NETWORK.to_owned()),
            BTreeMap::new(),
        ),
    };
    Entry {
        image: unit.image.clone(),
        profiles: vec![unit.profile.clone()],
        restart: RESTART,
        user: (unit.user == UserMode::Uid).then(|| {
            format!(
                "{}:{}",
                operated(values, PUID_KEY, OPERATOR_FALLBACK),
                operated(values, PGID_KEY, OPERATOR_FALLBACK)
            )
        }),
        command: unit.command.iter().map(|word| literal(word)).collect(),
        environment: environment(unit, values),
        env_file: unit
            .secrets_file
            .iter()
            .map(|path| literal(&path.display().to_string()))
            .collect(),
        ports: if matches!(placed, Placed::On(_)) {
            ports(unit, values)
        } else {
            Vec::new()
        },
        volumes: volumes(unit, values),
        tmpfs: tmpfs(unit.tmpfs, unit.takes_data)
            .map(|path| literal(path))
            .collect(),
        networks: Joined(
            networks
                .into_iter()
                .map(|(name, at)| (literal(&name), at))
                .collect(),
        ),
        network_mode,
        depends_on,
        security_opt: SECURITY,
        cap_drop: DROPPED,
        cap_add: if confined {
            Vec::new()
        } else {
            KEPT.into_iter().chain(granted(unit.shape)).collect()
        },
        devices: passed(unit.shape).chain(render_devices).collect(),
        group_add: render_groups,
        pids_limit: PROCESSES,
        read_only: confined,
        mem_limit: unit.mem_limit_mib.map(|mib| format!("{mib}m")),
    }
}

/// The kernel capabilities `shape` adds to the ones every service that is not confined
/// keeps.
pub(crate) fn granted(shape: Option<Shape>) -> impl Iterator<Item = &'static str> {
    shape
        .into_iter()
        .flat_map(|shape| shape.grants().iter().copied())
}

/// Each device `shape` passes in that needs nothing of the host beside the device, as
/// `host:container`: every one but the render devices, which wait on the host's render
/// group.
pub(crate) fn passed(shape: Option<Shape>) -> impl Iterator<Item = String> {
    shape
        .filter(|shape| *shape != Shape::Render)
        .into_iter()
        .flat_map(|shape| shape.devices().iter())
        .map(|device| format!("{device}:{device}"))
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
/// the time zone and the operator's pair where it asks for it, which no manifest overrides.
fn environment(unit: &Unit<'_>, values: &Values<'_>) -> BTreeMap<String, String> {
    let mut written: BTreeMap<String, String> = unit
        .env
        .iter()
        .filter_map(|(name, value)| {
            let text = valued(unit, value, values)?;
            Some((name.clone(), literal(&text)))
        })
        .collect();
    written.insert(
        ZONE_KEY.to_owned(),
        operated(values, ZONE_KEY, DEFAULT_ZONE),
    );
    if unit.user == UserMode::PuidEnv {
        for key in [PUID_KEY, PGID_KEY] {
            written.insert(key.to_owned(), operated(values, key, OPERATOR_FALLBACK));
        }
    }
    written
}

/// The operator's value for `key`, or `fallback` while their environment file names
/// none, as the document writes it.
fn operated(values: &Values<'_>, key: &str, fallback: &str) -> String {
    literal(
        &(values.operator)(key)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| fallback.to_owned()),
    )
}

/// What one variable is written as, or nothing where it has no value to write: a secret,
/// an address nothing fills, or a setting whose needs the other settings leave unmet.
fn valued(unit: &Unit<'_>, value: &Value, values: &Values<'_>) -> Option<String> {
    match value {
        Value::Constant(constant) => Some(constant.text()),
        Value::Setting(setting) => {
            let held = |named: &str| {
                (values.setting)(unit.namespace, named)
                    .or_else(|| choices::defaulted(unit.env, named))
            };
            choices::met(&setting.needs, held).then(|| {
                (values.setting)(unit.namespace, &setting.setting)
                    .unwrap_or_else(|| setting.default.text())
            })
        }
        Value::Url(url) => (values.address)(unit.id, url),
        Value::Secret(_) => None,
    }
}

/// Every port it publishes on its interface, then an adapter's contract port.
pub(crate) fn ports(unit: &Unit<'_>, values: &Values<'_>) -> Vec<String> {
    let household = operated(values, LAN_BIND_KEY, HOUSEHOLD);
    unit.published
        .iter()
        .map(|(tier, host, inside)| format!("{}:{host}:{inside}", tier.interface(&household)))
        .chain(
            unit.contract
                .map(|listens| format!("{LOOPBACK}::{listens}")),
        )
        .collect()
}

/// The library where it asked for it, its own configuration directory, and each path
/// mounted from its own mount tree.
fn volumes(unit: &Unit<'_>, values: &Values<'_>) -> Vec<String> {
    let service = literal(unit.id);
    let mut volumes = Vec::new();
    if unit.takes_data {
        let from = match values.nfs {
            Some(_) => LIBRARY_VOLUME.to_owned(),
            None => operated(values, DATA_ROOT_KEY, LIBRARY),
        };
        volumes.push(format!("{from}:{LIBRARY_TARGET}"));
    }
    volumes.push(mounted(
        &format!("./{CONFIGURATION_TREE}/{service}"),
        configuration(unit.config_path),
        unit.config_ro,
    ));
    if let Some((fronted, to)) = unit.shares.filter(|(_, to)| is_target(to, unit.takes_data)) {
        volumes.push(mounted(
            &format!("./{CONFIGURATION_TREE}/{}", literal(fronted)),
            to,
            false,
        ));
    }
    volumes.extend(
        mounts(unit.mounts, unit.takes_data).map(|Mount { from, to, ro }| {
            mounted(
                &format!("./{MOUNT_TREE}/{service}/{}", literal(from)),
                to,
                *ro,
            )
        }),
    );
    volumes
}

/// Where a configuration directory lands: where it is declared, where that is a target
/// the model allows, and the model's own default otherwise.
pub(crate) fn configuration(declared: &str) -> &str {
    if is_target(declared, true) {
        declared
    } else {
        CONFIGURATION
    }
}

/// Each further mount the model allows: a source inside its own mount tree and inside no
/// other of its sources, landing where a mount may land.
pub(crate) fn mounts(all: &[Mount], takes_data: bool) -> impl Iterator<Item = &Mount> {
    all.iter().filter(move |mount| {
        is_source(&mount.from)
            && is_target(&mount.to, takes_data)
            && !all
                .iter()
                .any(|other| lies_inside(&mount.from, &other.from))
    })
}

/// Each scratch path in memory the model allows.
pub(crate) fn tmpfs(all: &[String], takes_data: bool) -> impl Iterator<Item = &String> {
    all.iter().filter(move |path| is_target(path, takes_data))
}

/// One bind mount of `source` at `to`.
fn mounted(source: &str, to: &str, ro: bool) -> String {
    let mode = if ro { ":ro" } else { "" };
    format!("{source}:{}{mode}", literal(to))
}

/// A value Compose reads back as exactly itself.
#[must_use]
pub fn literal(text: &str) -> String {
    text.replace('$', "$$")
}
