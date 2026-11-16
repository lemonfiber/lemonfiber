//! The project's document: every service's entry, then every network the topology
//! declares, each one lemonfiber creates through the engine before a start and the
//! document names as existing.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::entry::{literal, Entry, CONFIGURATION_TREE, INSIDE, LIBRARY_VOLUME, MOUNT_TREE};
use crate::config::storage::Export;
use crate::topology::{Network, Topology};

/// `document` as the YAML Compose reads, or nothing where it cannot be written.
pub(crate) fn yaml(document: &impl Serialize) -> String {
    serde_norway::to_string(document).unwrap_or_default()
}

/// Every profile a service of the document `text` sits in.
pub(crate) fn profiles(text: &str) -> std::collections::BTreeSet<String> {
    let document: serde_norway::Value = serde_norway::from_str(text).unwrap_or_default();
    document
        .get("services")
        .and_then(serde_norway::Value::as_mapping)
        .into_iter()
        .flat_map(serde_norway::Mapping::values)
        .filter_map(|entry| entry.get("profiles")?.as_sequence())
        .flatten()
        .filter_map(|profile| profile.as_str().map(str::to_owned))
        .collect()
}

/// The services named in `names` of the document `text`, as the document writes them,
/// under its own `services` heading.
pub(crate) fn sliced(text: &str, names: &[&str]) -> String {
    let document: serde_norway::Value = serde_norway::from_str(text).unwrap_or_default();
    let services: serde_norway::Mapping = document
        .get("services")
        .and_then(serde_norway::Value::as_mapping)
        .map(|services| {
            services
                .iter()
                .filter(|(name, _)| name.as_str().is_some_and(|name| names.contains(&name)))
                .map(|(name, entry)| (name.clone(), entry.clone()))
                .collect()
        })
        .unwrap_or_default();
    let mut sliced = serde_norway::Mapping::new();
    sliced.insert("services".into(), services.into());
    yaml(&sliced)
}

/// Each directory a service of the document `text` mounts from the configuration or mount
/// tree beneath the project directory, by the service it is mounted into, as the engine
/// is handed it.
pub(crate) fn sources(text: &str) -> Vec<(String, String)> {
    let document: serde_norway::Value = serde_norway::from_str(text).unwrap_or_default();
    document
        .get("services")
        .and_then(serde_norway::Value::as_mapping)
        .into_iter()
        .flatten()
        .flat_map(|(name, entry)| {
            let service = name.as_str().unwrap_or_default().to_owned();
            entry
                .get("volumes")
                .and_then(serde_norway::Value::as_sequence)
                .into_iter()
                .flatten()
                .filter_map(|volume| volume.as_str()?.split(':').next())
                .filter(|source| {
                    [CONFIGURATION_TREE, MOUNT_TREE].iter().any(|tree| {
                        source
                            .strip_prefix("./")
                            .and_then(|beneath| beneath.strip_prefix(tree))
                            .is_some_and(|rest| rest.starts_with('/'))
                    })
                })
                .map(move |source| (service.clone(), source.replace("$$", "$")))
        })
        .collect()
}

/// The options every NFS library volume is mounted with, after the server's address:
/// version 4, read and write, and blocking until a dropped server returns rather than
/// handing a service a short read it would record as a corrupt import.
const NFS_OPTIONS: &str = "nfsvers=4,rw,hard,timeo=600,retrans=2";

/// The document: its name, services, then networks, then the library's volume.
#[derive(Serialize)]
pub(super) struct Document {
    /// The project's name.
    pub(super) name: String,
    /// Every service, in the order written.
    pub(super) services: Services,
    /// Every network, by name.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) networks: BTreeMap<String, Declared>,
    /// The library's volume, where it is kept on an NFS export and something takes it.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) volumes: BTreeMap<&'static str, Volume>,
}

/// A named volume the document declares.
#[derive(Debug, Serialize)]
pub(crate) struct Volume {
    /// The driver that mounts it.
    driver: &'static str,
    /// What the driver is told.
    driver_opts: Mounted,
}

/// What the local driver is told to mount.
#[derive(Debug, Serialize)]
struct Mounted {
    /// The filesystem.
    #[serde(rename = "type")]
    kind: &'static str,
    /// The mount options, the server's address first.
    o: String,
    /// The export, as the mount names it.
    device: String,
}

/// The library's volume, where `nfs` keeps it and `taken` says something mounts it.
pub(crate) fn library(nfs: Option<&Export>, taken: bool) -> BTreeMap<&'static str, Volume> {
    nfs.filter(|_| taken)
        .map(|export| {
            (
                LIBRARY_VOLUME,
                Volume {
                    driver: "local",
                    driver_opts: Mounted {
                        kind: "nfs",
                        o: format!("addr={},{NFS_OPTIONS}", literal(&export.server)),
                        device: format!(":{}", literal(&export.path)),
                    },
                },
            )
        })
        .into_iter()
        .collect()
}

/// Services in the order they were written: the stack's in its manifest's order, then
/// each plugin's in the register's.
pub(super) struct Services(pub(super) Vec<(String, Entry)>);

impl Serialize for Services {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.iter().map(|(name, entry)| (name, entry)))
    }
}

/// One network as the project declares it: one lemonfiber created, by the engine's name.
#[derive(Serialize)]
pub(super) struct Declared {
    /// Whether it exists already, which it always does.
    external: bool,
    /// The engine's name for it.
    name: String,
}

/// Every network `topology` declares, as the document writes it for `project`.
pub(super) fn networks(topology: &Topology, project: &str) -> BTreeMap<String, Declared> {
    topology
        .networks
        .keys()
        .map(|declared| {
            (
                literal(declared),
                Declared {
                    external: true,
                    name: literal(&named(project, declared)),
                },
            )
        })
        .collect()
}

/// The engine's name for the network `declared` of `project`.
#[must_use]
pub fn named(project: &str, declared: &str) -> String {
    format!("{project}_{declared}")
}

/// What `network` is made of, as lemonfiber creates it through the engine.
#[must_use]
pub fn shaped(network: &Network) -> lemonfiber_ports::docker::Shape {
    lemonfiber_ports::docker::Shape {
        subnet: network
            .subnet
            .map(|(first, prefix)| format!("{first}/{prefix}")),
        ip_range: network
            .fixed
            .map(|fixed| format!("{}/{}", fixed.handed_out.0, fixed.handed_out.1)),
        internal: network.internal,
        masquerade: network.internal || network.masquerade,
    }
}

/// The keys of an entry whose change creates nothing again: what orders and selects
/// services, and the networks a container is connected to where it runs.
const UNDIGESTED: [&str; 3] = ["depends_on", "profiles", "networks"];

/// One network a service joins: the engine's name for it, the names it answers to there,
/// its own first, and its fixed address there.
pub(crate) type Joined = (String, Vec<String>, Option<String>);

/// What a written project and the operator's overlays say of one service.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Written {
    /// Each entry for it, the project's first, as JSON apart from [`UNDIGESTED`].
    pub(crate) entries: Vec<String>,
    /// Every secrets file each entry names.
    pub(crate) secrets_files: Vec<String>,
    /// Each network it joins.
    pub(crate) networks: Vec<Joined>,
    /// Each service it waits for.
    pub(crate) depends: Vec<String>,
    /// The image it runs, as the last entry naming one names it.
    pub(crate) image: Option<String>,
    /// The service whose network it runs inside, as the last entry naming one names it.
    pub(crate) inside: Option<String>,
}

/// What the project `document` and the overlays `overlays` say of each service they
/// declare, the networks named as the engine names them for `project`.
pub(crate) fn written(
    document: &str,
    overlays: &[String],
    project: &str,
) -> BTreeMap<String, Written> {
    let parsed: Vec<serde_norway::Value> = std::iter::once(document)
        .chain(overlays.iter().map(String::as_str))
        .map(|text| serde_norway::from_str(text).unwrap_or_default())
        .collect();
    let names = parsed
        .first()
        .map(|first| engine_names(first, project))
        .unwrap_or_default();
    let services: BTreeSet<String> = parsed
        .iter()
        .flat_map(|source| source.get("services").map(keys).unwrap_or_default())
        .collect();
    services
        .into_iter()
        .map(|service| {
            let entries: Vec<&serde_norway::Value> = parsed
                .iter()
                .filter_map(|source| source.get("services")?.get(service.as_str()))
                .collect();
            let one = Written {
                entries: entries.iter().map(|entry| digested(entry)).collect(),
                secrets_files: entries
                    .iter()
                    .flat_map(|entry| secrets_files(entry))
                    .collect(),
                networks: entries
                    .iter()
                    .filter_map(|entry| entry.get("networks"))
                    .flat_map(|networks| joins(networks, &service, &names))
                    .collect(),
                depends: entries
                    .iter()
                    .filter_map(|entry| entry.get("depends_on"))
                    .flat_map(keys)
                    .collect(),
                image: entries
                    .iter()
                    .filter_map(|entry| entry.get("image")?.as_str())
                    .next_back()
                    .map(str::to_owned),
                inside: entries
                    .iter()
                    .filter_map(|entry| entry.get("network_mode")?.as_str())
                    .next_back()
                    .and_then(|mode| mode.strip_prefix(INSIDE))
                    .map(str::to_owned),
            };
            (service, one)
        })
        .collect()
}

/// The engine's name for each network the project `document` declares for `project`.
pub(crate) fn networks_named(document: &str, project: &str) -> BTreeSet<String> {
    let parsed: serde_norway::Value = serde_norway::from_str(document).unwrap_or_default();
    engine_names(&parsed, project).into_values().collect()
}

/// `entry` as JSON apart from [`UNDIGESTED`].
fn digested(entry: &serde_norway::Value) -> String {
    let mut kept = entry.clone();
    if let Some(mapping) = kept.as_mapping_mut() {
        for key in UNDIGESTED {
            mapping.remove(key);
        }
    }
    serde_json::to_string(&kept).unwrap_or_default()
}

/// Every secrets file `entry` names, written as one path, as several, or as several each
/// with its path beside whether it is required.
fn secrets_files(entry: &serde_norway::Value) -> Vec<String> {
    match entry.get("env_file") {
        Some(serde_norway::Value::String(path)) => vec![path.clone()],
        Some(serde_norway::Value::Sequence(files)) => files
            .iter()
            .filter_map(|file| file.as_str().or_else(|| file.get("path")?.as_str()))
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// The names `value` holds as a list or as the keys of a mapping.
fn keys(value: &serde_norway::Value) -> Vec<String> {
    match value {
        serde_norway::Value::Sequence(names) => names
            .iter()
            .filter_map(serde_norway::Value::as_str)
            .map(str::to_owned)
            .collect(),
        serde_norway::Value::Mapping(mapping) => mapping
            .keys()
            .filter_map(serde_norway::Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// Each network `networks` joins `service` to, by the engine's name in `names`, with the
/// names it answers to there, its own first, and its fixed address there.
fn joins(
    networks: &serde_norway::Value,
    service: &str,
    names: &BTreeMap<String, String>,
) -> Vec<Joined> {
    keys(networks)
        .into_iter()
        .map(|declared| {
            let held = networks.get(declared.as_str());
            let aliases = std::iter::once(service.to_owned())
                .chain(
                    held.and_then(|held| held.get("aliases"))
                        .map(keys)
                        .unwrap_or_default(),
                )
                .collect();
            let address = held
                .and_then(|held| held.get("ipv4_address"))
                .and_then(serde_norway::Value::as_str)
                .map(str::to_owned);
            let name = names.get(&declared).cloned().unwrap_or(declared);
            (name, aliases, address)
        })
        .collect()
}

/// The engine's name for each network `document` declares, by its declared name.
fn engine_names(document: &serde_norway::Value, project: &str) -> BTreeMap<String, String> {
    let declared = document.get("networks");
    declared
        .map(keys)
        .unwrap_or_default()
        .into_iter()
        .map(|name| {
            let engine = declared
                .and_then(|networks| networks.get(name.as_str())?.get("name")?.as_str())
                .map_or_else(|| named(project, &name), str::to_owned);
            (name, engine)
        })
        .collect()
}
