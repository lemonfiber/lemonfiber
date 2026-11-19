//! Each service on this machine as a filler: the stack's own, and each installed plugin's.

use std::collections::BTreeMap;
use std::path::Path;

use lemonfiber_manifest::Service;

use super::{Address, Filler};
use crate::origin::Origin;
use crate::plugin::{Installed, Placed};

/// One of the stack's own services, as something reaching it needs it.
pub(super) fn bundled(service: &Service, services: &[Service], project: Option<&Path>) -> Filler {
    Filler {
        id: service.id.clone(),
        name: service.name.clone(),
        origin: Origin::Bundled,
        address: service.listens.map(|port| Address {
            host: host(service, services),
            port,
        }),
        published: service.port,
        contract: None,
        confined_to: project
            .map(|project| crate::app::targets::service_config_dir(project, &service.id)),
        config_at: None,
        media_types: service.media_types.clone(),
        provides: service.provides.clone(),
        contracts: Vec::new(),
        first_party: false,
        granted: Vec::new(),
        minted: None,
        majors: service.majors(),
        fronts: None,
        upstream: BTreeMap::new(),
        native: None,
        replaces: None,
        via: service
            .network
            .as_ref()
            .and_then(|network| network.via.clone()),
        fenced: service.fenced(),
    }
}

/// One installed plugin's service, as something reaching it needs it.
///
/// Reached at its own id until [`Fillers::trusting`] places one taking `network.via` at
/// the service whose network it runs inside.
pub(super) fn brought(
    one: &Installed,
    placed: &Placed,
    project: Option<&Path>,
    first_party: bool,
) -> Filler {
    let plugin = one.plugin.as_str();
    Filler {
        id: placed.service.clone(),
        name: placed.called().to_owned(),
        origin: Origin::Plugin {
            named: plugin.to_owned(),
        },
        address: placed.listens.map(|port| Address {
            host: placed.service.clone(),
            port,
        }),
        published: placed.published(),
        contract: placed.contract_port(),
        confined_to: project
            .and_then(|project| crate::app::targets::plugin_config_dir(project, placed)),
        config_at: Some(crate::project::entry::configuration(&placed.config_path).to_owned()),
        media_types: placed.media_types.clone(),
        provides: placed.provides.clone(),
        contracts: placed.speaks.clone(),
        first_party,
        granted: one
            .granted
            .iter()
            .map(|grant| grant.credential.clone())
            .collect(),
        minted: one
            .declared
            .secrets
            .iter()
            .find(|secret| secret.minted && secret.of == placed.service)
            .map(|secret| crate::settings::named(plugin, &secret.id)),
        majors: lemonfiber_manifest::majors(&placed.tag),
        fronts: placed.fronts.clone(),
        upstream: placed.upstream_held(plugin),
        native: placed.native.clone(),
        replaces: placed.replaces.clone(),
        via: placed
            .fields
            .network
            .as_ref()
            .and_then(|network| network.via.clone()),
        fenced: placed
            .fields
            .network
            .as_ref()
            .is_some_and(|network| network.fenced),
    }
}

/// The name the stack's network resolves a bundled service to: its own id, or the id of
/// the service whose network it runs inside.
fn host(service: &Service, services: &[Service]) -> String {
    service
        .inside(services)
        .map_or_else(|| service.id.clone(), |host| host.id.clone())
}
