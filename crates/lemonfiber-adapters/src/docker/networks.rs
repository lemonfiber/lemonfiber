//! The networks a Compose project made, read and taken away over the Engine API.

use std::collections::HashMap;

use async_trait::async_trait;
use bollard::models::NetworkDisconnectRequest;
use bollard::query_parameters::ListNetworksOptionsBuilder;

use lemonfiber_ports::docker::{Failure, Network, Networks};

use super::{Daemon, PROJECT_LABEL};

/// The label Compose puts on every network it creates, naming it as the project's file
/// declares it.
const NETWORK_LABEL: &str = "com.docker.compose.network";

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
        Ok(listed
            .into_iter()
            .filter_map(|network| {
                let declared = network.labels?.get(NETWORK_LABEL)?.clone();
                Some(Network {
                    name: network.name?,
                    declared,
                })
            })
            .collect())
    }

    async fn removed(&self, name: &str) -> Result<(), Failure> {
        removed(self, name).await
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
