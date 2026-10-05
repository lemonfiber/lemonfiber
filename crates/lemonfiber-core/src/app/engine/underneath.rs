//! Refusing to start a stack from a container whose paths the machine does not share.
//!
//! Compose resolves every bind mount on the machine the engine runs on, never in the
//! container that asked. A copy of lemonfiber in a container hands Compose the paths
//! it sees — the stack's own directory, which every relative path in every fragment
//! is resolved against, and the data root. Where the machine has those at the same
//! paths, what Compose resolves is what lemonfiber meant. Where it has them somewhere
//! else, the engine mounts whatever the machine keeps at the path it was given,
//! which is usually nothing, and makes an empty directory there.
//!
//! So each path is put to the engine before anything runs, as the machine path behind
//! the container path, and a start is refused unless the two are the same path. The
//! engine is asked because it made the mounts; nothing read inside the container can
//! say which host path a NAS share came from.
//!
//! Every answer that is not a proof refuses. An engine that cannot be reached is the
//! socket not being mounted, and one that does not know this container is a different
//! engine from the one running it; either way the check cannot be made, and a start
//! that went ahead on that would be the silent failure this exists to stop.

use std::path::{Path, PathBuf};

use crate::app::Ctx;
use crate::config::Settings;
use crate::contained::behind;
use crate::error::codes::life::{ELSEWHERE_UNDERNEATH, NOT_ON_THIS_ENGINE, NO_ENGINE_IN_HERE};
use crate::error::{Diagnose, Problem, Remedy, Severity, State};
use crate::ports::docker::Failure;
use crate::stack::Source;

/// Where the host's Docker socket is mounted into the container, as the templates
/// mount it.
const SOCKET: &str = "/var/run/docker.sock";

/// Whether the machine under this container has the stack's paths at the same paths.
///
/// `Ok(())` outside a container, which is every ordinary run, and with a remote engine
/// in force, whose machine the remote check already asks about directly.
///
/// # Errors
///
/// Returns the [`Problem`] naming both paths where a path the stack mounts is
/// somewhere else on the machine or nowhere on it, and the one saying the check could
/// not be made where the engine could not be reached or does not know this container.
pub(crate) async fn verified(ctx: &Ctx) -> Result<(), Box<Problem>> {
    let Some(container) = ctx.settings.container.as_deref() else {
        return Ok(());
    };
    if ctx.settings.docker.is_remote() {
        return Ok(());
    }
    let mounts = match ctx.seams.locations.mounted(container).await {
        Ok(Some(mounts)) => mounts,
        Ok(None) => return Err(Box::new(unseen(container))),
        Err(failure) => return Err(Box::new(unreached(&failure))),
    };
    for path in mounted(ctx.stack, &ctx.settings) {
        let host = behind(&mounts, &path);
        if host.as_deref() != Some(path.as_path()) {
            return Err(Box::new(elsewhere(&path, host.as_deref())));
        }
    }
    Ok(())
}

/// The paths this stack hands Compose that the engine resolves on the machine.
///
/// The stack's directory, wherever it is — the operator's own, or where the embedded
/// one is written — and the data root, where one has been chosen. A path not yet
/// chosen is setup's question rather than this one's.
fn mounted(stack: Source, settings: &Settings) -> Vec<PathBuf> {
    let stack = match stack {
        Source::External(path) => Some(path.to_path_buf()),
        Source::Embedded(_) => settings.stack_dir.clone(),
    };
    stack
        .into_iter()
        .chain(settings.data_root.clone())
        .collect()
}

/// What to tell an operator whose container sees a path the machine keeps elsewhere,
/// or does not keep at all.
///
/// Names both paths, because either alone looks right: the container's is where the
/// files plainly are, and the machine's is what the template was told to mount.
///
/// The paths go in the remedy's own words rather than in its detail. Detail is read
/// for credentials and a `host:container` pair is shaped like a setting, so a NAS
/// share under `/mnt/user` would arrive withheld.
fn elsewhere(path: &Path, host: Option<&Path>) -> Problem {
    let seen = path.display();
    let (summary, meaning, remedy) = match host {
        Some(host) => {
            let host = host.display();
            (
                format!("{seen} in this container is {host} on the machine"),
                format!(
                    "lemonfiber runs in a container, and Compose resolves every path the stack \
                     mounts on the machine underneath it. The stack would hand it {seen}, which \
                     this container sees at {host} on the machine — so the engine would mount \
                     whatever the machine keeps at {seen} instead, or make an empty directory \
                     there. Nothing was started."
                ),
                Remedy::new(format!(
                    "Mount {host} at {host} inside the container, the same path inside and out, \
                     and point lemonfiber at that path"
                )),
            )
        }
        None => (
            format!("{seen} is not mounted into this container from the machine"),
            format!(
                "lemonfiber runs in a container, and Compose resolves every path the stack \
                 mounts on the machine underneath it. {seen} exists only inside this container, \
                 so the machine has nothing there and the engine would make an empty directory \
                 in its place. Nothing was started."
            ),
            Remedy::new(format!(
                "Mount a directory from the machine at {seen}, the same path inside and out"
            )),
        ),
    };
    Problem::new(
        ELSEWHERE_UNDERNEATH,
        Severity::Error,
        summary,
        meaning,
        remedy,
    )
    .in_state(State::Guided)
}

/// What to tell an operator whose container has no way to Docker.
///
/// The engine's own account goes underneath, because it says whether the socket is
/// missing or was refused, and those are fixed differently.
fn unreached(failure: &Failure) -> Problem {
    Problem::new(
        NO_ENGINE_IN_HERE,
        Severity::Error,
        "lemonfiber cannot reach Docker from inside this container",
        format!(
            "lemonfiber drives the stack through the host's Docker socket, which the templates \
             mount at {SOCKET}, and nothing answered there. Nothing was started."
        ),
        Remedy::new("Mount the host's Docker socket into the container, as the templates do")
            .with_detail(format!("-v {SOCKET}:{SOCKET}")),
    )
    .in_state(State::Guided)
    .caused_by(failure.problem())
}

/// What to tell an operator whose container reaches an engine that is not running it.
fn unseen(container: &str) -> Problem {
    Problem::new(
        NOT_ON_THIS_ENGINE,
        Severity::Error,
        "the Docker lemonfiber reaches is not the one running its container",
        format!(
            "lemonfiber runs in container {container}, and the engine at the socket it was given \
             has no container by that name. So which machine paths stand behind the paths this \
             container sees cannot be asked, and Compose would resolve them somewhere nobody \
             checked. Nothing was started."
        ),
        Remedy::new(format!(
            "Mount the socket of the engine running this container at {SOCKET}"
        )),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests;
