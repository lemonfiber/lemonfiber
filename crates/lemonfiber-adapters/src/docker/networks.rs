//! The networks of a project, created, read and taken away over the Engine API.
//!
//! Each is labelled as Compose labels a network it creates, so a network Compose created
//! for an earlier start reads the same as one created here.

use std::collections::HashMap;

use async_trait::async_trait;
use bollard::models::{
    EndpointIpamConfig, EndpointSettings, Ipam, IpamConfig, NetworkConnectRequest,
    NetworkCreateRequest, NetworkDisconnectRequest,
};
use bollard::query_parameters::ListNetworksOptionsBuilder;

use lemonfiber_ports::docker::{Endpoint, Failure, Joining, Network, Networks, Shape, Wanted};

use super::{Daemon, PROJECT_LABEL};

/// The label naming a network as the project declares it.
const NETWORK_LABEL: &str = "com.docker.compose.network";

/// The driver every network of a project runs on.
const BRIDGE: &str = "bridge";

/// The bridge driver's option saying whether it translates addresses off the host.
const MASQUERADE: &str = "com.docker.network.bridge.enable_ip_masquerade";

/// What [`MASQUERADE`] holds where it translates none.
const NO_MASQUERADE: &str = "false";

/// The fewest characters of a container's id the engine abbreviates it to.
const SHORT_ID: usize = 12;

#[async_trait]
impl Networks for Daemon {
    async fn networks(&self, project: &str) -> Result<Vec<Network>, Failure> {
        let docker = self.client().await?;
        let filters = HashMap::from([("label", vec![format!("{PROJECT_LABEL}={project}")])]);
        let listed = docker
            .list_networks(Some(
                ListNetworksOptionsBuilder::new().filters(&filters).build(),
            ))
            .await
            .map_err(|error| self.refused(&error))?;
        Ok(listed.into_iter().filter_map(read).collect())
    }

    async fn created(&self, wanted: Wanted<'_>) -> Result<(), Failure> {
        let docker = self.client().await?;
        docker
            .create_network(request(wanted))
            .await
            .map(|_| ())
            .map_err(|error| self.refused(&error))
    }

    async fn removed(&self, name: &str) -> Result<(), Failure> {
        removed(self, name).await
    }

    async fn connected(&self, joining: Joining<'_>) -> Result<(), Failure> {
        let docker = self.client().await?;
        docker
            .connect_network(joining.network, connection(joining))
            .await
            .map_err(|error| self.refused(&error))
    }

    async fn disconnected(&self, network: &str, container: &str) -> Result<(), Failure> {
        let docker = self.client().await?;
        docker
            .disconnect_network(
                network,
                NetworkDisconnectRequest {
                    container: container.to_owned(),
                    force: Some(true),
                },
            )
            .await
            .map_err(|error| self.refused(&error))
    }

    async fn endpoints(&self, container: &str) -> Result<Vec<Endpoint>, Failure> {
        endpoints(self, container).await
    }
}

/// How the container `container` is on each network it is on, as `daemon`'s engine says,
/// leaving out each name it answers to that is its own name or an abbreviation of its id.
async fn endpoints(daemon: &Daemon, container: &str) -> Result<Vec<Endpoint>, Failure> {
    let docker = daemon.client().await?;
    let inspected = docker
        .inspect_container(container, None)
        .await
        .map_err(|error| daemon.refused(&error))?;
    let name = inspected.name.unwrap_or_default();
    let own = name.trim_start_matches('/');
    let id = inspected.id.unwrap_or_default();
    let given = |alias: &String| {
        alias != own && !(alias.len() >= SHORT_ID && id.starts_with(alias.as_str()))
    };
    Ok(inspected
        .network_settings
        .and_then(|settings| settings.networks)
        .unwrap_or_default()
        .into_iter()
        .map(|(network, settings)| Endpoint {
            network,
            address: settings
                .ipam_config
                .and_then(|ipam| ipam.ipv4_address)
                .filter(|address| !address.is_empty()),
            aliases: settings
                .aliases
                .unwrap_or_default()
                .into_iter()
                .filter(given)
                .collect(),
        })
        .collect())
}

/// What the engine is asked to connect `joining` as.
fn connection(joining: Joining<'_>) -> NetworkConnectRequest {
    NetworkConnectRequest {
        container: joining.container.to_owned(),
        endpoint_config: Some(EndpointSettings {
            aliases: Some(joining.aliases.to_vec()),
            ipam_config: joining.address.map(|address| EndpointIpamConfig {
                ipv4_address: Some(address.to_owned()),
                ..EndpointIpamConfig::default()
            }),
            ..EndpointSettings::default()
        }),
    }
}

/// Disconnect every container still on the network `name`, then remove it.
async fn removed(daemon: &Daemon, name: &str) -> Result<(), Failure> {
    let docker = daemon.client().await?;
    let inspected = docker
        .inspect_network(name, None)
        .await
        .map_err(|error| daemon.refused(&error))?;
    for container in inspected.containers.unwrap_or_default().into_keys() {
        docker
            .disconnect_network(
                name,
                NetworkDisconnectRequest {
                    container,
                    force: Some(true),
                },
            )
            .await
            .map_err(|error| daemon.refused(&error))?;
    }
    docker
        .remove_network(name)
        .await
        .map_err(|error| daemon.refused(&error))
}

/// The network the engine listed as `network`, where it names itself and the project's
/// name for it.
fn read(network: bollard::models::Network) -> Option<Network> {
    let labels = network.labels.unwrap_or_default();
    let declared = labels.get(NETWORK_LABEL)?.clone();
    let pool = network
        .ipam
        .and_then(|ipam| ipam.config)
        .and_then(|config| config.into_iter().next())
        .unwrap_or_default();
    let options = network.options.unwrap_or_default();
    Some(Network {
        name: network.name?,
        declared,
        id: network.id.unwrap_or_default(),
        shape: Shape {
            subnet: pool.subnet,
            ip_range: pool.ip_range,
            internal: network.internal.unwrap_or_default(),
            masquerade: options.get(MASQUERADE).map(String::as_str) != Some(NO_MASQUERADE),
        },
        gateway: pool.gateway,
    })
}

/// What the engine is asked to create `wanted` as.
fn request(wanted: Wanted<'_>) -> NetworkCreateRequest {
    let shape = wanted.shape;
    NetworkCreateRequest {
        name: wanted.name.to_owned(),
        driver: Some(BRIDGE.to_owned()),
        internal: Some(shape.internal),
        ipam: shape.subnet.as_ref().map(|subnet| Ipam {
            config: Some(vec![IpamConfig {
                subnet: Some(subnet.clone()),
                ip_range: shape.ip_range.clone(),
                ..IpamConfig::default()
            }]),
            ..Ipam::default()
        }),
        options: (!shape.masquerade)
            .then(|| HashMap::from([(MASQUERADE.to_owned(), NO_MASQUERADE.to_owned())])),
        labels: Some(HashMap::from([
            (PROJECT_LABEL.to_owned(), wanted.project.to_owned()),
            (NETWORK_LABEL.to_owned(), wanted.declared.to_owned()),
        ])),
        ..NetworkCreateRequest::default()
    }
}

#[cfg(test)]
mod tests;
