//! Asking a daemon about a path on its own machine, and reading what it says.
//!
//! There is no route that stats a host path. Everything the API touches on the
//! machine under it, it touches through a container — so the question has to be
//! asked as a request to make one, and the answer read out of the refusal.
//!
//! A create request is refused in a fixed order, and the mounts are checked before
//! the image is looked for. So a request naming an image that cannot exist can only
//! come back two ways: refused for the mount, which says the path is not there, or
//! refused for the image, which says the daemon got past the mount and therefore
//! that it is. Nothing is created on either path, nothing is pulled, and there is
//! nothing to clean up afterwards — both outcomes are failures, which is the whole
//! reason this shape was chosen over one that makes a container and removes it.
//!
//! What it does *not* establish is that the path is a directory. A file where a
//! directory belongs reads as present here, where a shell asking `test -d` would
//! have said no. That case is a loud refusal from Docker at the moment of mounting;
//! the one this exists to prevent is the silent one, where an absent path becomes an
//! empty directory and the stack comes up on nothing.
//!
//! The order this rests on is the daemon's, not a promise. Whoever calls this is
//! expected to prove it still holds before believing a positive — see the guard that
//! does, which is where that belongs, because it is a decision rather than a reading.

use std::path::Path;

use bollard::models::{ContainerCreateBody, HostConfig, Mount, MountType};
use lemonfiber_ports::docker::{Failure, Presence};

use super::Daemon;

/// The image a probe names, which no machine can have.
///
/// An identifier rather than a repository name, so that nothing anywhere can go
/// looking for it: a name would be a thing a client could try to pull, and a probe
/// that reached a registry would be a pre-flight that needed the internet. All
/// zeroes is not the digest of any content, and cannot become one.
const NOTHING: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

/// Where the probe would have put the path, had any of this been real.
///
/// Arbitrary, and never mounted anywhere — but it has to be *somewhere*, because a
/// mount with no target is refused for the target rather than for the source, which
/// would make every answer mean nothing.
const SOMEWHERE: &str = "/lemonfiber-preflight";

/// The status a daemon refuses a mount configuration it cannot honour with.
const UNMOUNTABLE: u16 = 400;

/// The status a daemon answers with once it is past the mounts and looking for an
/// image it has not got.
const NO_SUCH_IMAGE: u16 = 404;

/// The daemon's own words for the condition this whole check exists to find.
const NOT_THERE: &str = "bind source path does not exist";

/// Put the question to the daemon, and read its refusal as the answer.
///
/// Outside the method for the reason every decision here is: `#[async_trait]`
/// rewrites a body into a generated future the coverage report attributes nothing
/// to, so a branch left in there could go untaken for ever without the gate saying
/// a word.
pub(super) async fn looked(daemon: &Daemon, path: &Path) -> Result<Presence, Failure> {
    let asked = daemon
        .client()
        .await?
        .create_container(
            None::<bollard::query_parameters::CreateContainerOptions>,
            asking(&path.display().to_string()),
        )
        .await;

    match asked {
        // The daemon answered about the request, which is where the answer is.
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code,
            message,
        }) => Ok(read(status_code, &message)),
        // Nothing usable came back: no daemon, or one whose reply this cannot read.
        // Either way it is a fact about reaching the machine rather than about
        // anything on it, and it keeps the distinctions the connection already draws
        // between a name, a port and a key.
        Err(error) => Err(daemon.refused(&error)),
        // The request names an image that cannot exist, so this cannot happen — and
        // a probe that somehow made a container is one that no longer knows what it
        // is measuring, which is the single thing it must not report as a yes.
        Ok(_) => Ok(Presence::Unknown),
    }
}

/// The request that asks a daemon whether `path` is on its machine.
///
/// Read-only, because a probe has no business being able to write even in a world
/// where it accidentally succeeded.
pub(super) fn asking(path: &str) -> ContainerCreateBody {
    ContainerCreateBody {
        image: Some(NOTHING.to_owned()),
        host_config: Some(HostConfig {
            mounts: Some(vec![Mount {
                typ: Some(MountType::BIND),
                source: Some(path.to_owned()),
                target: Some(SOMEWHERE.to_owned()),
                read_only: Some(true),
                ..Mount::default()
            }]),
            ..HostConfig::default()
        }),
        ..ContainerCreateBody::default()
    }
}

/// What a daemon's refusal says about the path it was asked about.
///
/// Both halves of the mount refusal are required, rather than the status alone: a
/// request can be refused for its target, for a propagation mode, for a dozen things
/// that are not a statement about the source. Reading any of those as an absent path
/// would refuse a machine that is perfectly ready.
///
/// Anything else is [`Presence::Unknown`], including the wording having changed —
/// which is the case the caller's own check is there to catch, because a reading that
/// has stopped recognising a "no" would otherwise turn every "yes" into a certainty.
pub(super) fn read(status: u16, said: &str) -> Presence {
    match status {
        UNMOUNTABLE if said.contains(NOT_THERE) => Presence::Absent,
        NO_SUCH_IMAGE => Presence::There,
        _ => Presence::Unknown,
    }
}

#[cfg(test)]
mod tests;
