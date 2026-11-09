//! Who an adapter runs as: the owner of what lemonfiber writes for it, which is the
//! owner of the stack directory, since the key and the configuration an adapter reads
//! are written beneath it and nowhere else.
//!
//! An adapter reads nothing but its own configuration, so it runs as whoever wrote that,
//! whatever `PUID` and `PGID` say; a service that speaks no contract runs as them where
//! the operator set them, and as this same owner otherwise. Root is never the user.
//! Where lemonfiber runs as root, the fallback user is run as instead, and an adapter's
//! key and configuration are handed to that user.

use std::path::Path;

use crate::project::entry::OPERATOR_FALLBACK;

/// The user and group root is handed over to.
const HANDED_TO: (u32, u32) = (OPERATOR_FALLBACK, OPERATOR_FALLBACK);

/// The user and group an adapter writes its files as and runs as, for an owner of
/// `owner`: the owner itself, or the fallback user where that is root.
#[must_use]
pub(crate) const fn as_adapter(owner: (u32, u32)) -> (u32, u32) {
    if owner.0 == 0 {
        HANDED_TO
    } else {
        owner
    }
}

/// The user and group an adapter of the stack at `stack` runs as, or nothing where the
/// stack directory names no owner.
#[must_use]
pub(crate) fn running_as(stack: &Path) -> Option<(u32, u32)> {
    owner(stack).map(as_adapter)
}

/// Hand `path`, a file or a directory lemonfiber wrote for an adapter of the stack at
/// `stack`, to the user that adapter runs as, where that is not whoever owns it now: only
/// ever where lemonfiber runs as root.
///
/// # Errors
///
/// Why it could not be handed over.
pub(crate) fn handed_over(path: &Path, stack: &Path) -> Result<(), String> {
    let (Some(now), Some(to)) = (owner(path), running_as(stack)) else {
        return Ok(());
    };
    if now == to {
        return Ok(());
    }
    chowned(path, to)
}

/// `root` and everything beneath it given to `to`, no link followed: a lent adapter's
/// throwaway configuration, where whoever wrote it is not who the adapter runs as.
///
/// # Errors
///
/// Why something beneath it could not be read or handed over.
pub(crate) fn handed_over_whole(root: &Path, to: (u32, u32)) -> Result<(), String> {
    chowned(root, to)?;
    let is_directory = std::fs::symlink_metadata(root)
        .map_err(|why| format!("{} could not be read: {why}", root.display()))?
        .is_dir();
    if !is_directory {
        return Ok(());
    }
    let entries = std::fs::read_dir(root)
        .map_err(|why| format!("{} could not be read: {why}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|why| format!("{} could not be read: {why}", root.display()))?;
        handed_over_whole(&entry.path(), to)?;
    }
    Ok(())
}

/// The user and group owning `path`, read without following a link.
#[cfg(unix)]
fn owner(path: &Path) -> Option<(u32, u32)> {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::symlink_metadata(path)
        .ok()
        .map(|held| (held.uid(), held.gid()))
}

/// Nobody, where the platform names no owner.
#[cfg(not(unix))]
fn owner(_path: &Path) -> Option<(u32, u32)> {
    None
}

/// `path` given to `to`, without following a link.
#[cfg(unix)]
fn chowned(path: &Path, (uid, gid): (u32, u32)) -> Result<(), String> {
    std::os::unix::fs::lchown(path, Some(uid), Some(gid)).map_err(|why| {
        format!(
            "{} could not be handed to {uid}:{gid}: {why}",
            path.display()
        )
    })
}

/// Nothing to hand over, where the platform names no owner.
#[cfg(not(unix))]
fn chowned(_path: &Path, _to: (u32, u32)) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests;
