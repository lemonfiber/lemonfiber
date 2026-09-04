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

use crate::ports::filesystem::Fault;
use crate::ports::occupancy::{Occupancy, Occupant};

use super::filesystem::Disk;

#[async_trait]
impl Occupancy for Disk {
    async fn beneath(&self, root: &Path) -> Result<Vec<Occupant>, Fault> {
        // The root is opened on its own, because it is the one refusal an operator
        // has to hear about: a tree that is not there yet is the ordinary first-run
        // state and counts as nothing, while one that is there and will not be read
        // must not be reported as an empty disk.
        let top = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(Fault::new(error.to_string())),
        };

        let mut found = Vec::new();
        let mut pending = Vec::new();
        for entry in top.flatten() {
            gather(&entry, &mut pending, &mut found);
        }
        // Below the root, a directory that will not open is a gap in the count
        // rather than a failed reading: one unreadable folder must not lose the
        // answer for everything beside it, so what cannot be opened contributes
        // nothing and the walk carries on.
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(&directory)
                .into_iter()
                .flatten()
                .flatten()
            {
                gather(&entry, &mut pending, &mut found);
            }
        }
        found.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(found)
    }
}

/// One directory entry: descended into where it is a directory of its own, and
/// counted where it is anything else.
fn gather(entry: &std::fs::DirEntry, pending: &mut Vec<PathBuf>, found: &mut Vec<Occupant>) {
    // The metadata of the entry itself rather than of what it points at, so a
    // symbolic link is the link and never the tree at the far end of it. An entry
    // whose metadata will not read is passed over — it is a name that was there a
    // moment ago and is not now, which is a gap in a count rather than a failure.
    for meta in entry.metadata() {
        if meta.is_dir() {
            pending.push(entry.path());
        } else {
            found.push(Occupant {
                path: entry.path(),
                bytes: meta.len(),
                identity: Some(super::filesystem::identity_of(&meta)),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::adapters::Disk;
    use crate::ports::occupancy::Occupancy;

    #[tokio::test]
    async fn a_tree_that_is_not_there_holds_nothing_rather_than_failing() {
        // The ordinary first-run state: a data location named before anything was
        // written into it.
        let walked = Disk
            .beneath(Path::new("/a-path-nothing-has-ever-been-at"))
            .await;
        assert_eq!(walked, Ok(Vec::new()));
    }

    #[tokio::test]
    async fn every_file_beneath_a_real_tree_is_found_with_its_size_and_identity() {
        let root = std::env::temp_dir().join(format!("lemonfiber-walk-{}", std::process::id()));
        let nested = root.join("under");
        let _ = std::fs::create_dir_all(&nested);
        let _ = std::fs::write(root.join("top.txt"), "0123456789");
        let _ = std::fs::write(nested.join("deep.txt"), "abc");

        let walked = Disk.beneath(&root).await.unwrap_or_default();
        let names: Vec<String> = walked
            .iter()
            .filter_map(|occupant| {
                occupant
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .collect();
        assert_eq!(
            names,
            ["deep.txt", "top.txt"],
            "read in one order every run"
        );
        assert_eq!(
            walked.iter().map(|occupant| occupant.bytes).sum::<u64>(),
            13
        );
        assert!(
            walked.iter().all(|occupant| occupant
                .identity
                .is_some_and(|identity| identity.links >= 1)),
            "each name says which file it points at"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_file_named_as_the_root_is_a_root_with_nothing_beneath_it() {
        let root = std::env::temp_dir().join(format!("lemonfiber-file-{}", std::process::id()));
        let _ = std::fs::write(&root, "x");
        // Reading a file as a directory is not a "not there", so it reaches the
        // operator as the platform's own words rather than as an empty answer.
        assert!(Disk.beneath(&root).await.is_err());
        let _ = std::fs::remove_file(&root);
    }
}
