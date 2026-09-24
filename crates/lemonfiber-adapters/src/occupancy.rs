//! Walking a real directory tree.
//!
//! Translation, and no decisions: what is under the root, how large each file is,
//! and which underlying file each name points at. What any of that means — which
//! tree it belongs to, whether two names are one file, whether it is waste — is
//! decided above the port, where a fake can drive every case of it.
//!
//! The walk is its own rather than a crate's, for the reason the rest of this
//! module is thin: what a directory walker would be asked to do here is
//! `read_dir` and recurse, and a dependency to do that is a dependency to keep
//! current.
//!
//! Nothing is followed out of the tree. A symbolic link is reported as the file it
//! is rather than descended into, so a link pointing back up cannot walk forever
//! and a link pointing at somebody else's disk cannot make their files count
//! against this one. A hardlink is not a link in that sense and is walked
//! normally — it is a second name for a file that is genuinely here, which is the
//! whole thing the counting above exists to get right.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use lemonfiber_ports::filesystem::Fault;
use lemonfiber_ports::occupancy::{Occupancy, Occupant};

use super::filesystem::Disk;

#[async_trait]
impl Occupancy for Disk {
    /// Handed to a thread that is allowed to block, whole rather than call by call.
    ///
    /// A walk of a real library is thousands of `read_dir` and `metadata` calls, and
    /// every one of them blocks — on the runtime's own thread that is every other task
    /// waiting. Handed over once because a walk is one long operation: making each
    /// syscall its own handover would pay for the crossing thousands of times to
    /// answer a question that was never going to be answered in between.
    async fn beneath(&self, root: &Path) -> Result<Vec<Occupant>, Fault> {
        let root = root.to_path_buf();
        tokio::task::spawn_blocking(move || walked(&root))
            .await
            .map_err(fault)?
    }
}

/// A fault carrying whatever refused, in its own words.
fn fault(refusal: impl std::fmt::Display) -> Fault {
    Fault::new(refusal.to_string())
}

/// Everything under a root, walked on a thread that may block.
fn walked(root: &Path) -> Result<Vec<Occupant>, Fault> {
    // The root is opened on its own, because it is the one refusal an operator
    // has to hear about: a tree that is not there yet is the ordinary first-run
    // state and counts as nothing, while one that is there and will not be read
    // must not be reported as an empty disk.
    let top = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(fault(error)),
    };

    let mut found = Vec::new();
    let mut pending = Vec::new();
    sort_into(top, &mut pending, &mut found);
    // Below the root, a directory that will not open is a gap in the count
    // rather than a failed reading: one unreadable folder must not lose the
    // answer for everything beside it, so what cannot be opened contributes
    // nothing and the walk carries on.
    while let Some(directory) = pending.pop() {
        let opened = std::fs::read_dir(&directory).into_iter().flatten();
        sort_into(opened, &mut pending, &mut found);
    }
    found.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(found)
}

/// Put each entry where it belongs: a directory onto the list still to walk, a
/// file onto the count.
fn sort_into(
    entries: impl Iterator<Item = std::io::Result<std::fs::DirEntry>>,
    pending: &mut Vec<PathBuf>,
    found: &mut Vec<Occupant>,
) {
    for gathered in entries.flatten().filter_map(|entry| gathered(&entry)) {
        match gathered {
            Ok(directory) => pending.push(directory),
            Err(occupant) => found.push(occupant),
        }
    }
}

/// What one directory entry is: a directory to descend into, or a file to count.
///
/// The metadata of the entry itself rather than of what it points at, so a
/// symbolic link is the link and never the tree at the far end of it. An entry
/// whose metadata will not read is neither — a name that was there a moment ago
/// and is not now, which is a gap in a count rather than a failure.
fn gathered(entry: &std::fs::DirEntry) -> Option<Result<PathBuf, Occupant>> {
    let meta = entry.metadata().ok()?;
    Some(if meta.is_dir() {
        Ok(entry.path())
    } else {
        Err(Occupant {
            path: entry.path(),
            bytes: meta.len(),
            identity: Some(super::filesystem::identity_of(&meta)),
        })
    })
}

#[cfg(test)]
mod tests;
