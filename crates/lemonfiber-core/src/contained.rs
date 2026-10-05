//! Whether this copy of lemonfiber runs inside a container, and what that container
//! sees of the machine under it.
//!
//! Compose resolves every bind mount on the machine the engine runs on, never in the
//! container that asked. So a copy of lemonfiber in a container that hands Compose a
//! path it sees has handed over a path the machine may not have, or may have as
//! something else entirely. Two facts settle whether that is so: which container this
//! is, and which machine path stands behind each path it sees.
//!
//! The first is read off the container's own mount table. An engine gives every
//! container its own `/etc/hostname`, bind-mounted from a directory named after the
//! container, so the table names the container in the root of that mount. That holds
//! under Docker, rootless Docker and Podman alike, and on any version of control
//! groups, which is why it is read rather than the hostname — a hostname is
//! whatever the template set — or the control-group file, which says nothing useful
//! once a machine is on the unified hierarchy.
//!
//! The second is asked of the engine, which made the mounts. Both are pure functions
//! here over what was read, so every arrangement is reachable from a test.

use std::path::{Component, Path, PathBuf};

use crate::ports::docker::Mount;

/// Where a Linux process reads its own mount table.
pub const MOUNT_TABLE: &str = "/proc/self/mountinfo";

/// The files an engine bind-mounts into every container from that container's own
/// directory.
const ENGINE_WRITTEN: &[&str] = &["/etc/hostname", "/etc/hosts", "/etc/resolv.conf"];

/// How long an engine's container identifier is, in hexadecimal digits.
const IDENTIFIER: usize = 64;

/// The container this process runs in, from the text of `/proc/self/mountinfo`.
///
/// Nothing where none of the files an engine writes is mounted from a directory named
/// after a container, which is every machine that is not a container — a host's own
/// `/etc/hostname` is a file on its root filesystem rather than a mount.
#[must_use]
pub fn container(mountinfo: &str) -> Option<String> {
    mountinfo.lines().find_map(|line| {
        let mut fields = line.split(' ');
        let root = fields.nth(3)?;
        let point = fields.next()?;
        ENGINE_WRITTEN
            .contains(&point)
            .then(|| identified(root))
            .flatten()
    })
}

/// The container identifier a mount's root names, where it names one.
///
/// An identifier counts only directly under a directory whose name ends in
/// `containers`, which is where every engine keeps them: Docker's `containers/<id>`
/// and Podman's `overlay-containers/<id>`. Sixty-four hexadecimal digits anywhere
/// else in a path is a coincidence rather than a container.
fn identified(root: &str) -> Option<String> {
    let parts: Vec<&str> = root.split('/').collect();
    parts.windows(2).find_map(|pair| match pair {
        [parent, id] if parent.ends_with("containers") && is_identifier(id) => {
            Some((*id).to_owned())
        }
        _ => None,
    })
}

/// Whether a name is shaped like an engine's container identifier.
fn is_identifier(name: &str) -> bool {
    name.len() == IDENTIFIER && name.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The machine path behind a path this container sees, where a mount accounts for it.
///
/// The deepest mount holding the path wins, because a mount inside another mount
/// hides what was beneath it. Nothing where no mount holds it, which means the path
/// is in the container's own layer and the machine has nothing there at all.
#[must_use]
pub fn behind(mounts: &[Mount], path: &Path) -> Option<PathBuf> {
    mounts
        .iter()
        .filter_map(|mount| {
            let rest = path.strip_prefix(&mount.destination).ok()?;
            Some((depth(&mount.destination), mount.source.join(rest)))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, host)| host)
}

/// How many directories deep a path is.
fn depth(path: &Path) -> usize {
    path.components()
        .filter(|part| matches!(part, Component::Normal(_)))
        .count()
}

#[cfg(test)]
mod tests;
