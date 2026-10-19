//! Serving an image as its service runs, through the engine API: a network of its own,
//! a container on it with its one port on the loopback interface, read, and removed.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use bollard::container::LogOutput;
use bollard::models::{
    ContainerCreateBody, ContainerInspectResponse, EndpointSettings, HealthStatusEnum, HostConfig,
    Mount, MountType, NetworkCreateRequest, NetworkingConfig, PortBinding, SystemInfo,
};
use bollard::query_parameters::{
    ListNetworksOptionsBuilder, LogsOptionsBuilder, RemoveContainerOptionsBuilder,
};
use bollard::Docker;
use tokio_stream::StreamExt as _;

use lemonfiber_ports::program::{Unran, LEFT_BEHIND};
use lemonfiber_ports::serving::{Serve, Served, Standing};

use super::program::{refused, uncreated, PROOF_LABEL};
use super::Daemon;

/// What every network this starts is named beginning with, before the moment it was made.
const NAMED: &str = "lemonfiber-record-";

/// The interface the one port is published on.
const LOOPBACK: &str = "127.0.0.1";

/// Told apart across the networks one process makes.
static COUNTED: AtomicU64 = AtomicU64::new(0);

/// Make a network and serve `serve` on it, removing both where the container cannot start.
pub(super) async fn served(daemon: &Daemon, serve: &Serve) -> Result<Served, Unran> {
    let docker = engine(daemon).await?;
    let machine = docker
        .info()
        .await
        .map(|info| machine(&info))
        .map_err(|error| refused(&error))?;
    super::program::swept(docker).await;
    swept(docker, SystemTime::now()).await;
    let network = named(SystemTime::now());
    docker
        .create_network(NetworkCreateRequest {
            name: network.clone(),
            driver: Some("bridge".to_owned()),
            internal: Some(false),
            attachable: Some(false),
            labels: Some(labelled()),
            ..NetworkCreateRequest::default()
        })
        .await
        .map_err(|error| refused(&error))?;
    let started = started(docker, serve, &network, machine).await;
    if started.is_err() {
        let _ = docker.remove_network(&network).await;
    }
    started
}

/// Serve `serve` on the network `served` is on.
pub(super) async fn beside(
    daemon: &Daemon,
    served: &Served,
    serve: &Serve,
) -> Result<Served, Unran> {
    let docker = engine(daemon).await?;
    started(docker, serve, &served.network, served.machine.clone()).await
}

/// One container for `serve` on `network`, started and read, or removed where it could
/// not be.
async fn started(
    docker: &Docker,
    serve: &Serve,
    network: &str,
    machine: Vec<String>,
) -> Result<Served, Unran> {
    let container = docker
        .create_container(None, body(serve, network))
        .await
        .map_err(|error| uncreated(&error, &serve.image))?
        .id;
    let inspected = match docker.start_container(&container, None).await {
        Ok(()) => docker
            .inspect_container(&container, None)
            .await
            .map_err(|error| refused(&error)),
        Err(error) => Err(refused(&error)),
    };
    match inspected {
        Ok(inspected) => Ok(Served {
            machine,
            ..read_off(&inspected, &container, network, serve.listens)
        }),
        Err(unran) => {
            gone(docker, &container, network).await;
            Err(unran)
        }
    }
}

/// How the container stands now.
pub(super) async fn standing(daemon: &Daemon, served: &Served) -> Result<Standing, Unran> {
    let docker = engine(daemon).await?;
    let inspected = docker
        .inspect_container(&served.container, None)
        .await
        .map_err(|error| refused(&error))?;
    Ok(stood(&inspected))
}

/// The last `lines` lines the container wrote, both streams together.
pub(super) async fn logged(daemon: &Daemon, served: &Served, lines: usize) -> String {
    let Ok(docker) = engine(daemon).await else {
        return String::new();
    };
    let options = LogsOptionsBuilder::default()
        .stdout(true)
        .stderr(true)
        .follow(false)
        .tail(&lines.to_string())
        .build();
    let mut stream = docker.logs(&served.container, Some(options));
    let mut text = Vec::new();
    while let Some(Ok(chunk)) = stream.next().await {
        if let LogOutput::StdOut { message } | LogOutput::StdErr { message } = chunk {
            text.extend_from_slice(&message);
        }
    }
    String::from_utf8_lossy(&text).into_owned()
}

/// Take the container off the engine, and its network once nothing else is on it.
pub(super) async fn removed(daemon: &Daemon, served: &Served) {
    if let Ok(docker) = engine(daemon).await {
        gone(docker, &served.container, &served.network).await;
    }
}

/// The engine's client, or why it could not be reached.
async fn engine(daemon: &Daemon) -> Result<&Docker, Unran> {
    daemon.client().await.map_err(|failure| Unran::Unreachable {
        reason: failure.to_string(),
    })
}

/// The labels every network and container this starts carries.
fn labelled() -> HashMap<String, String> {
    HashMap::from([(PROOF_LABEL.to_owned(), "record".to_owned())])
}

/// A fresh network's name, carrying the moment it was made.
pub(super) fn named(now: SystemTime) -> String {
    let since = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    format!(
        "{NAMED}{since}-{}-{}",
        std::process::id(),
        COUNTED.fetch_add(1, Ordering::Relaxed)
    )
}

/// The port as the engine keys it.
fn keyed(listens: u16) -> String {
    format!("{listens}/tcp")
}

/// What the engine is asked to create for `serve`, on `network`.
pub(super) fn body(serve: &Serve, network: &str) -> ContainerCreateBody {
    let published = serve.listens.map(|listens| {
        HashMap::from([(
            keyed(listens),
            Some(vec![PortBinding {
                host_ip: Some(LOOPBACK.to_owned()),
                host_port: Some(String::new()),
            }]),
        )])
    });
    ContainerCreateBody {
        networking_config: serve.alias.as_ref().map(|alias| NetworkingConfig {
            endpoints_config: Some(HashMap::from([(
                network.to_owned(),
                EndpointSettings {
                    aliases: Some(vec![alias.clone()]),
                    ..EndpointSettings::default()
                },
            )])),
        }),
        image: Some(serve.image.clone()),
        env: Some(serve.env.clone()),
        user: serve.user.clone(),
        attach_stdin: Some(false),
        tty: Some(false),
        open_stdin: Some(false),
        exposed_ports: serve.listens.map(|listens| vec![keyed(listens)]),
        labels: Some(labelled()),
        host_config: Some(HostConfig {
            network_mode: Some(network.to_owned()),
            port_bindings: published,
            security_opt: Some(serve.security_opt.clone()),
            cap_drop: Some(serve.cap_drop.clone()),
            cap_add: Some(serve.cap_add.clone()),
            pids_limit: Some(serve.pids_limit),
            readonly_rootfs: Some(serve.read_only),
            memory: serve.memory,
            privileged: Some(false),
            publish_all_ports: Some(false),
            tmpfs: Some(
                serve
                    .tmpfs
                    .iter()
                    .map(|path| (path.clone(), String::new()))
                    .collect(),
            ),
            mounts: Some(
                serve
                    .binds
                    .iter()
                    .map(|bound| Mount {
                        typ: Some(MountType::BIND),
                        source: Some(bound.source.display().to_string()),
                        target: Some(bound.target.clone()),
                        read_only: Some(false),
                        ..Mount::default()
                    })
                    .collect(),
            ),
            ..HostConfig::default()
        }),
        ..ContainerCreateBody::default()
    }
}

/// What the engine says of a started container: its name, its addresses and where the
/// loopback interface reaches its port.
pub(super) fn read_off(
    inspected: &ContainerInspectResponse,
    container: &str,
    network: &str,
    listens: Option<u16>,
) -> Served {
    let settings = inspected.network_settings.as_ref();
    let addresses = settings
        .and_then(|settings| settings.networks.as_ref())
        .map(|networks| {
            networks
                .values()
                .flat_map(|endpoint| {
                    [
                        &endpoint.ip_address,
                        &endpoint.global_ipv6_address,
                        &endpoint.mac_address,
                    ]
                })
                .flatten()
                .filter(|address| !address.is_empty())
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let port = listens.and_then(|listens| {
        settings
            .and_then(|settings| settings.ports.as_ref())
            .and_then(|ports| ports.get(&keyed(listens)))
            .and_then(Option::as_ref)
            .and_then(|bindings| {
                bindings
                    .iter()
                    .filter(|binding| binding.host_ip.as_deref() == Some(LOOPBACK))
                    .find_map(|binding| binding.host_port.as_deref()?.parse().ok())
            })
    });
    Served {
        container: container.to_owned(),
        network: network.to_owned(),
        hostname: inspected
            .config
            .as_ref()
            .and_then(|config| config.hostname.clone())
            .unwrap_or_default(),
        addresses,
        machine: Vec::new(),
        port,
    }
}

/// What the engine says of the machine it runs on: its name, kernel version and operating
/// system, each that it names.
pub(super) fn machine(info: &SystemInfo) -> Vec<String> {
    [&info.name, &info.kernel_version, &info.operating_system]
        .into_iter()
        .flatten()
        .filter(|said| !said.is_empty())
        .cloned()
        .collect()
}

/// How an inspected container stands.
pub(super) fn stood(inspected: &ContainerInspectResponse) -> Standing {
    let Some(state) = inspected.state.as_ref() else {
        return Standing::Exited(-1);
    };
    if state.running != Some(true) {
        return Standing::Exited(state.exit_code.unwrap_or(-1));
    }
    match state.health.as_ref().and_then(|health| health.status) {
        Some(HealthStatusEnum::STARTING) => Standing::Starting,
        Some(HealthStatusEnum::HEALTHY) => Standing::Healthy,
        Some(HealthStatusEnum::UNHEALTHY) => Standing::Unhealthy,
        _ => Standing::Running,
    }
}

/// Take a container off the engine, and its network once nothing else is on it.
async fn gone(docker: &Docker, container: &str, network: &str) {
    let options = RemoveContainerOptionsBuilder::default()
        .force(true)
        .v(true)
        .build();
    let _ = docker.remove_container(container, Some(options)).await;
    let _ = docker.remove_network(network).await;
}

/// Remove every network a recording before this one left behind; their containers go
/// with the sweep every proof's run makes.
async fn swept(docker: &Docker, now: SystemTime) {
    let filters = HashMap::from([("label".to_owned(), vec![PROOF_LABEL.to_owned()])]);
    let options = ListNetworksOptionsBuilder::default()
        .filters(&filters)
        .build();
    let Ok(listed) = docker.list_networks(Some(options)).await else {
        return;
    };
    let names: Vec<String> = listed
        .into_iter()
        .filter_map(|network| network.name)
        .collect();
    for name in left_behind(&names, now) {
        let _ = docker.remove_network(&name).await;
    }
}

/// Every network named as this names them, made longer ago than any recording lasts.
pub(super) fn left_behind(names: &[String], now: SystemTime) -> Vec<String> {
    let Ok(since) = now.duration_since(SystemTime::UNIX_EPOCH) else {
        return Vec::new();
    };
    let before = since.saturating_sub(LEFT_BEHIND).as_secs();
    names
        .iter()
        .filter(|name| {
            name.strip_prefix(NAMED)
                .and_then(|rest| rest.split('-').next())
                .and_then(|made| made.parse::<u64>().ok())
                .is_some_and(|made| made < before)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
