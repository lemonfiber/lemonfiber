//! Asking a daemon what a container has mounted, and from where.
//!
//! Read off the container's own description rather than off its mount table. The
//! table inside a container names the filesystem a mount came from and the path
//! within it, which is the host's path only where that filesystem sits at the host's
//! root — and on a NAS it rarely does, because the shares are a filesystem of their
//! own mounted somewhere under `/mnt`. The daemon made the mount and says which host
//! path it used, so it is the daemon that is asked.

use bollard::models::{ContainerInspectResponse, MountPoint};
use lemonfiber_ports::docker::{Failure, Mount};

use super::Daemon;

/// The status a daemon answers with for a container it does not have.
const NO_SUCH_CONTAINER: u16 = 404;

/// Ask the daemon about the container, and keep only what says where its paths are.
///
/// Outside the method for the reason every decision in this adapter is: the body of
/// an `#[async_trait]` method becomes a generated future the coverage report
/// attributes nothing to.
pub(super) async fn inspected(
    daemon: &Daemon,
    container: &str,
) -> Result<Option<Vec<Mount>>, Failure> {
    let asked = daemon
        .client()
        .await?
        .inspect_container(
            container,
            None::<bollard::query_parameters::InspectContainerOptions>,
        )
        .await;

    match asked {
        Ok(described) => Ok(Some(mounts(described))),
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: NO_SUCH_CONTAINER,
            ..
        }) => Ok(None),
        Err(error) => Err(daemon.refused(&error)),
    }
}

/// Every mount in a description that names both of its ends.
///
/// A mount missing either end says nothing about where a path lives, and leaving it
/// out reads as the path not being mounted from it — which is the answer that
/// refuses rather than the one that lets a start through on a guess.
pub(super) fn mounts(described: ContainerInspectResponse) -> Vec<Mount> {
    described
        .mounts
        .unwrap_or_default()
        .into_iter()
        .filter_map(both_ends)
        .collect()
}

/// One mount, where the daemon said where it came from and where it went.
fn both_ends(point: MountPoint) -> Option<Mount> {
    Some(Mount {
        source: point.source.filter(|path| !path.is_empty())?.into(),
        destination: point.destination.filter(|path| !path.is_empty())?.into(),
    })
}

#[cfg(test)]
mod tests;
