//! The one Compose project lemonfiber generates: every bundled stack service and every
//! installed plugin's service, each written from the one container model and placed on
//! the networks [`crate::topology`] derives.
//!
//! A pure function of the stack's manifest, the plugin register, the operator's filler
//! choices and what [`Reading`] answers. Nothing here touches the disk or the engine.

mod document;
pub mod entry;
mod inputs;

use std::path::PathBuf;

use lemonfiber_manifest::container::{UrlOf, UserMode};
use lemonfiber_manifest::{Bind, Manifest, Service};

use crate::plugin::{self, Installed};
use crate::topology::{Placed, Topology};
use crate::wiring::Chosen;
use document::{Document, Services};
use entry::{configuration, entry, literal, ports, Tier, Unit, Values};
use inputs::Inputs;

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
}

impl Reading<'_> {
    /// Nothing recorded, no secrets file and no render devices.
    #[must_use]
    pub const fn bare() -> Reading<'static> {
        Reading {
            setting: &|_, _| None,
            secrets_file: &|_| None,
            render_group: None,
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
    };
    let units: Vec<Unit<'_>> = inputs::written(manifest)
        .map(|service| stack_unit(service))
        .chain(installed.iter().flat_map(|one| {
            let legacy = plugin::Reading {
                secrets_file: reading.secrets_file,
                ..plugin::Reading::bare()
            };
            one.services
                .iter()
                .map(move |placed| plugin::unit(&one.plugin, placed, &legacy))
        }))
        .collect();
    let mut entries: Vec<(String, _)> = units
        .iter()
        .map(|unit| {
            let placed = topology
                .placed
                .get(unit.id)
                .cloned()
                .unwrap_or(Placed::Nowhere);
            (literal(unit.id), entry(unit, &placed, &values))
        })
        .collect();
    for unit in &units {
        if let Some(Placed::Inside(host)) = topology.placed.get(unit.id) {
            let hosted = ports(unit);
            if let Some((_, written)) = entries.iter_mut().find(|(id, _)| *id == literal(host)) {
                written.ports.extend(hosted);
            }
        }
    }
    let document = Document {
        services: Services(entries),
        networks: document::networks(&topology),
    };
    Project {
        text: serde_norway::to_string(&document).unwrap_or_default(),
        topology,
    }
}

/// A stack service as its entry needs it.
fn stack_unit(service: &Service) -> Unit<'_> {
    let image = match &service.digest {
        Some(digest) => format!(
            "{}:{}@{}",
            literal(&service.image),
            literal(&service.tag),
            literal(digest)
        ),
        None => format!("{}:{}", literal(&service.image), literal(&service.tag)),
    };
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
        secrets_file: None,
    }
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
