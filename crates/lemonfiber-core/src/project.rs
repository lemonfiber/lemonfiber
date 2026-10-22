//! The one Compose project lemonfiber generates: every bundled stack service and every
//! installed plugin's service, each written from the one container model and placed on
//! the networks [`crate::topology`] derives.
//!
//! A pure function of the stack's manifest, the plugin register, the operator's filler
//! choices and what [`Reading`] answers. Nothing here touches the disk or the engine.

pub(crate) mod document;
pub mod entry;
mod inputs;
pub mod overlay;

use std::collections::BTreeMap;
use std::path::PathBuf;

use lemonfiber_manifest::container::{UrlOf, UserMode, Value};
use lemonfiber_manifest::{Bind, Manifest, Service};

use crate::config::storage::Export;
use crate::model::MovedReport;
use crate::plugin::{self, Installed};
use crate::topology::{Placed, Topology};
use crate::wiring::Chosen;
use document::{Document, Services};
use entry::{configuration, entry, literal, ports, Tier, Unit, Values};
pub(crate) use inputs::filler as placed_at;
use inputs::Inputs;
pub(crate) use inputs::{declared, moved_in_place, written};

/// The file the project is written to, beneath the project directory.
pub const FILE: &str = "compose.generated.yml";

/// What the project's values are read from beyond the manifest and the register.
pub struct Reading<'a> {
    /// The operator's value for a setting of the named namespace, where one is recorded.
    pub setting: &'a dyn Fn(&str, &str) -> Option<String>,
    /// The named service's secrets file, where it names a secret and has somewhere to
    /// keep one.
    pub secrets_file: &'a dyn Fn(&str) -> Option<PathBuf>,
    /// The group that owns the host's render devices, where the host has them.
    pub render_group: Option<u32>,
    /// The NFS export the library is kept on, where it is kept on one.
    pub nfs: Option<&'a Export>,
    /// Where standing beside an existing setup moved each service's published port.
    pub moved: &'a [MovedReport],
    /// The project's name, which Compose labels everything it makes for it with.
    pub name: &'a str,
    /// The operator's value for a variable of their environment file, where one is
    /// written: who the services run as, the time zone, where the library is kept and
    /// the interface the household tier is published on. Never a secret.
    pub operator: &'a dyn Fn(&str) -> Option<String>,
}

impl Reading<'_> {
    /// Nothing recorded, no secrets file, no render devices, the library on this machine,
    /// nothing moved and the project named for the product.
    #[must_use]
    pub const fn bare() -> Reading<'static> {
        Reading {
            setting: &|_, _| None,
            secrets_file: &|_| None,
            render_group: None,
            nfs: None,
            moved: &[],
            name: crate::PRODUCT,
            operator: &|_| None,
        }
    }
}

/// The generated project: its text and the networks it places every service on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// The document, as written.
    pub text: String,
    /// Every network and every service's place on them.
    pub topology: Topology,
}

/// The project `manifest`'s services and `installed`'s come to, wired as `chosen`
/// settles them.
#[must_use]
pub fn generated(
    manifest: &Manifest,
    installed: &[Installed],
    chosen: &Chosen,
    reading: &Reading<'_>,
) -> Project {
    let inputs = inputs::read(manifest, installed, chosen);
    let topology = inputs.topology();
    let address =
        |service: &str, url: &UrlOf| address(manifest, installed, &inputs, &topology, service, url);
    let values = Values {
        setting: reading.setting,
        address: &address,
        render_group: reading.render_group,
        nfs: reading.nfs,
        operator: reading.operator,
    };
    let units: Vec<Unit<'_>> = inputs::written(manifest, installed)
        .map(|service| stack_unit(service, reading))
        .chain(installed.iter().flat_map(|one| {
            let secrets = plugin::Reading {
                secrets_file: reading.secrets_file,
                ..plugin::Reading::bare()
            };
            one.services
                .iter()
                .map(move |placed| plugin::unit(&one.plugin, placed, &secrets))
        }))
        .map(|unit| beside(unit, reading.moved))
        .collect();
    let mut hosted: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for unit in &units {
        if let Some(Placed::Inside(host)) = topology.placed.get(unit.id) {
            hosted
                .entry(literal(host))
                .or_default()
                .extend(ports(unit, &values));
        }
    }
    let entries: Vec<(String, _)> = units
        .iter()
        .map(|unit| {
            let placed = topology
                .placed
                .get(unit.id)
                .cloned()
                .unwrap_or(Placed::Nowhere);
            let id = literal(unit.id);
            let mut written = entry(unit, &placed, &values);
            written.ports.extend(hosted.remove(&id).unwrap_or_default());
            (id, written)
        })
        .collect();
    let taken = units.iter().any(|unit| unit.takes_data);
    let document = Document {
        name: literal(reading.name),
        services: Services(entries),
        networks: document::networks(&topology),
        volumes: document::library(reading.nfs, taken),
    };
    Project {
        text: document::yaml(&document),
        topology,
    }
}

/// The entries the project writes for `installed`'s services beside the stack this build
/// carries, with every value read as `reading` answers it: what installing it adds to the
/// project.
#[must_use]
pub fn entries(installed: &Installed, reading: &Reading<'_>) -> String {
    let ours: Vec<&str> = installed
        .services
        .iter()
        .map(|placed| placed.service.as_str())
        .collect();
    Manifest::from_toml(lemonfiber_bundled::STACK)
        .map(|manifest| {
            let project = generated(
                &manifest,
                std::slice::from_ref(installed),
                &Chosen::read(None),
                reading,
            );
            document::sliced(&project.text, &ours)
        })
        .unwrap_or_default()
}

/// The networks `manifest`'s services and `installed`'s are placed on, wired as `chosen`
/// settles them.
#[must_use]
pub fn topology(manifest: &Manifest, installed: &[Installed], chosen: &Chosen) -> Topology {
    inputs::read(manifest, installed, chosen).topology()
}

/// The profiles of the services that `services` run inside, which have to be active for
/// Compose to start them: the one exception to a service starting only with its own.
#[must_use]
pub fn hosting(
    manifest: &Manifest,
    installed: &[Installed],
    topology: &Topology,
    services: &[String],
) -> std::collections::BTreeSet<String> {
    services
        .iter()
        .filter_map(|service| match topology.placed.get(service) {
            Some(Placed::Inside(host)) => Some(host),
            _ => None,
        })
        .filter_map(|host| {
            manifest
                .services
                .iter()
                .find(|service| &service.id == host)
                .map(|service| service.profile.clone())
                .or_else(|| {
                    installed
                        .iter()
                        .find(|one| one.services.iter().any(|placed| &placed.service == host))
                        .map(|one| plugin::profile(&one.plugin))
                })
        })
        .collect()
}

/// Whether a journalled change is a write of the project.
pub(crate) fn is_written(change: &crate::journal::Change) -> bool {
    match &change.kind {
        crate::journal::Kind::Rewritten { path, .. } | crate::journal::Kind::Made { path } => {
            std::path::Path::new(path).file_name() == Some(std::ffi::OsStr::new(FILE))
        }
        _ => false,
    }
}

/// `unit`, published where standing beside moved it rather than where it would be.
fn beside<'a>(unit: Unit<'a>, moved: &[MovedReport]) -> Unit<'a> {
    let published = unit
        .published
        .iter()
        .map(|&(tier, host, inside)| {
            let to = moved
                .iter()
                .find(|one| one.service == unit.id && one.from == host)
                .map_or(host, |one| one.to);
            (tier, to, inside)
        })
        .collect();
    Unit { published, ..unit }
}

/// A stack service as its entry needs it, with its own secrets file where it names a
/// secret.
fn stack_unit<'a>(service: &'a Service, reading: &Reading<'_>) -> Unit<'a> {
    let image = literal(&reference(service));
    let tier = match service.bind {
        Some(Bind::Lan) => Tier::Household,
        Some(Bind::Loopback) | None => Tier::Loopback,
    };
    let published = service
        .port
        .map(|port| (tier, port, service.listens.unwrap_or(port)))
        .into_iter()
        .chain(
            service
                .publish
                .iter()
                .map(|further| (tier, further.port, further.listens.unwrap_or(further.port))),
        )
        .collect();
    Unit {
        id: &service.id,
        namespace: &service.id,
        profile: service.profile.clone(),
        image,
        published,
        contract: None,
        takes_data: service.takes_data,
        config_path: configuration(service.config_path.as_deref().unwrap_or_default()),
        config_ro: service.config_ro,
        mounts: &service.mount,
        tmpfs: &service.tmpfs,
        env: &service.env,
        user: UserMode::of(service.user_mode, false, service.posture),
        posture: service.posture,
        mem_limit_mib: service.mem_limit_mib,
        shape: service.shape,
        secrets_file: names_a_secret(&service.env)
            .then(|| (reading.secrets_file)(&service.id))
            .flatten(),
    }
}

/// The image `service` runs, by its digest where it pins one.
pub(crate) fn reference(service: &Service) -> String {
    match &service.digest {
        Some(digest) => format!("{}:{}@{digest}", service.image, service.tag),
        None => format!("{}:{}", service.image, service.tag),
    }
}

/// Whether an environment names a secret, which is written to the service's own file.
pub(crate) fn names_a_secret(env: &BTreeMap<String, Value>) -> bool {
    env.values().any(|value| matches!(value, Value::Secret(_)))
}

/// The address `service`'s ask for `url` reaches, where it settles on one service: the
/// host that service's network is held by, at the port it listens on.
fn address(
    manifest: &Manifest,
    installed: &[Installed],
    inputs: &Inputs,
    topology: &Topology,
    service: &str,
    url: &UrlOf,
) -> Option<String> {
    let candidates: Vec<String> = if url.files.is_some() || url.native.is_some() {
        inputs::reaching(
            manifest,
            installed,
            &url.url_of,
            url.files.as_deref(),
            url.native.is_some(),
        )
    } else {
        inputs
            .asks
            .iter()
            .filter(|(asker, capability, _)| asker == service && *capability == url.url_of)
            .map(|(_, _, reached)| reached.clone())
            .collect()
    };
    let [reached] = candidates.as_slice() else {
        return None;
    };
    let port = listening(manifest, installed, reached)?;
    let host = match topology.placed.get(reached.as_str()) {
        Some(Placed::Inside(host)) => host.clone(),
        _ => reached.clone(),
    };
    Some(format!("http://{host}:{port}"))
}

/// The port the named service answers on inside the project's networks.
fn listening(manifest: &Manifest, installed: &[Installed], id: &str) -> Option<u16> {
    let bundled = manifest
        .services
        .iter()
        .find(|service| service.id == id)
        .map(|service| service.listens.or(service.port));
    let brought = || {
        installed
            .iter()
            .flat_map(|one| &one.services)
            .find(|placed| placed.service == id)
            .map(|placed| placed.listens.or_else(|| placed.published()))
    };
    bundled.or_else(brought).flatten()
}

#[cfg(test)]
mod tests;
