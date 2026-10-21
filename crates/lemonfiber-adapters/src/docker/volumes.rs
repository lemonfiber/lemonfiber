//! The volumes the engine keeps for a service, asked after and copied out over the
//! Engine API.
//!
//! A volume is reached through a container, so one is created with the volume mounted
//! read-only, never started, asked for an archive of the mount, and removed. The volume
//! itself is never written and never removed. The image the container is created from is
//! the service's own, pulled first where the engine has not got it.

use std::path::Path;

use async_trait::async_trait;
use bollard::models::{ContainerCreateBody, HostConfig, Mount, MountType};
use bollard::query_parameters::{
    CreateImageOptionsBuilder, DownloadFromContainerOptionsBuilder, RemoveContainerOptionsBuilder,
};
use bollard::Docker;
use tokio_stream::StreamExt as _;

use lemonfiber_ports::docker::{EngineVolumes, Failure};

use super::Daemon;

/// Where the volume is mounted in the container that reaches it.
const FROM: &str = "/from";

/// The status the engine answers a volume or an image it has not got with.
const NOT_THERE: u16 = 404;

#[async_trait]
impl EngineVolumes for Daemon {
    async fn kept(&self, name: &str) -> Result<bool, Failure> {
        kept(self, name).await
    }

    async fn copied(
        &self,
        name: &str,
        image: &str,
        into: &Path,
        within: &Path,
    ) -> Result<(), Failure> {
        let archive = handed_over(self, name, image).await?;
        crate::filesystem::unpacked(&archive, into, within).map_err(|fault| Failure::Unreachable {
            reason: fault.message,
        })
    }
}

/// Whether the engine keeps the volume `name`.
async fn kept(daemon: &Daemon, name: &str) -> Result<bool, Failure> {
    match daemon.client().await?.inspect_volume(name).await {
        Ok(_) => Ok(true),
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: NOT_THERE,
            ..
        }) => Ok(false),
        Err(error) => Err(daemon.refused(&error)),
    }
}

/// The archive of the volume `name`, read through a container of `image` that is created
/// for it, pulling the image first where the engine has not got it, and removed after.
async fn handed_over(daemon: &Daemon, name: &str, image: &str) -> Result<Vec<u8>, Failure> {
    let docker = daemon.client().await?;
    let created = match create(docker, name, image).await {
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: NOT_THERE,
            ..
        }) => {
            pulled(docker, image)
                .await
                .map_err(|error| daemon.refused(&error))?;
            create(docker, name, image).await
        }
        other => other,
    }
    .map_err(|error| daemon.refused(&error))?;
    let archive = archived(daemon, docker, &created.id).await;
    let options = RemoveContainerOptionsBuilder::default().force(true).build();
    let _ = docker.remove_container(&created.id, Some(options)).await;
    archive
}

/// Create the container that reaches the volume `name`, unstarted.
async fn create(
    docker: &Docker,
    name: &str,
    image: &str,
) -> Result<bollard::models::ContainerCreateResponse, bollard::errors::Error> {
    docker
        .create_container(
            None::<bollard::query_parameters::CreateContainerOptions>,
            reaching(name, image),
        )
        .await
}

/// The container that reaches the volume `name`: created from `image`, the volume
/// mounted read-only, and no network.
fn reaching(name: &str, image: &str) -> ContainerCreateBody {
    ContainerCreateBody {
        image: Some(image.to_owned()),
        network_disabled: Some(true),
        host_config: Some(HostConfig {
            mounts: Some(vec![Mount {
                typ: Some(MountType::VOLUME),
                source: Some(name.to_owned()),
                target: Some(FROM.to_owned()),
                read_only: Some(true),
                ..Mount::default()
            }]),
            network_mode: Some("none".to_owned()),
            ..HostConfig::default()
        }),
        ..ContainerCreateBody::default()
    }
}

/// Pull `image`, waiting for the engine to finish.
async fn pulled(docker: &Docker, image: &str) -> Result<(), bollard::errors::Error> {
    let options = CreateImageOptionsBuilder::default()
        .from_image(image)
        .build();
    let mut progress = docker.create_image(Some(options), None, None);
    while let Some(step) = progress.next().await {
        step?;
    }
    Ok(())
}

/// The archive of what the container `id` sees at the volume's mount, refused where it
/// is larger than lemonfiber unpacks.
async fn archived(daemon: &Daemon, docker: &Docker, id: &str) -> Result<Vec<u8>, Failure> {
    let options = DownloadFromContainerOptionsBuilder::default()
        .path(FROM)
        .build();
    let mut chunks = docker.download_from_container(id, Some(options));
    let mut archive = Vec::new();
    while let Some(chunk) = chunks.next().await {
        archive.extend_from_slice(&chunk.map_err(|error| daemon.refused(&error))?);
        bounded(
            archive.len(),
            crate::filesystem::UNPACKED_AT_MOST.saturating_mul(2),
        )?;
    }
    Ok(archive)
}

/// Refuse an archive of `held` bytes where it is more than `at_most`.
fn bounded(held: usize, at_most: u64) -> Result<(), Failure> {
    if held as u64 > at_most {
        return Err(Failure::Unreachable {
            reason: "the volume holds more than lemonfiber copies".to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
