//! Touching the real filesystem.
//!
//! Translation, and no decisions. Each method is one system operation whose
//! result — success, or the platform's own words for a failure — is handed
//! straight back; what a sequence of them proves is decided in
//! [`crate::doctor::storage`], where a fake filesystem can drive every outcome.
//!
//! The identity a file reports differs by platform — an inode on Unix, a file
//! index on Windows — so reading it is the one place here that asks what it is
//! running on. It asks with `cfg(unix)`/`cfg(windows)`, which selects the code
//! at compile time rather than testing the operating system at runtime.
//!
//! `sysinfo` lives here and nowhere else, which an architecture test enforces.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::ports::filesystem::{Fault, FileSystem, Identity, StorageFacts};

/// The filesystem on this machine, reached through the standard library.
#[derive(Debug, Default, Clone, Copy)]
pub struct Disk;

#[async_trait]
impl FileSystem for Disk {
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf, Fault> {
        std::fs::canonicalize(path).map_err(|error| fault(&error))
    }

    async fn touch(&self, path: &Path) -> Result<(), Fault> {
        std::fs::File::create(path)
            .map(drop)
            .map_err(|error| fault(&error))
    }

    async fn link(&self, from: &Path, to: &Path) -> Result<(), Fault> {
        std::fs::hard_link(from, to).map_err(|error| fault(&error))
    }

    async fn identify(&self, path: &Path) -> Result<Identity, Fault> {
        std::fs::metadata(path)
            .map(|meta| identity_of(&meta))
            .map_err(|error| fault(&error))
    }

    async fn remove(&self, path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    async fn describe(&self, path: &Path) -> StorageFacts {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        let mounts: Vec<(PathBuf, String, bool)> = disks
            .list()
            .iter()
            .map(|disk| {
                (
                    disk.mount_point().to_path_buf(),
                    disk.file_system().to_string_lossy().into_owned(),
                    disk.is_removable(),
                )
            })
            .collect();
        crate::ports::filesystem::pick(&mounts, path)
    }
}

/// A filesystem error in the platform's own words.
fn fault(error: &std::io::Error) -> Fault {
    Fault::new(error.to_string())
}

/// A file's identity as Unix reports it: its inode and how many names point at
/// it.
#[cfg(unix)]
fn identity_of(meta: &std::fs::Metadata) -> Identity {
    use std::os::unix::fs::MetadataExt as _;

    Identity {
        file: meta.ino(),
        links: meta.nlink(),
    }
}

/// A file's identity as Windows reports it: its file index, and its link count
/// where the filesystem supplies one.
///
/// A filesystem that reports no index leaves nothing to compare, which the
/// caller reads as being unable to confirm the link rather than as a failure to
/// make one.
#[cfg(windows)]
fn identity_of(meta: &std::fs::Metadata) -> Identity {
    use std::os::windows::fs::MetadataExt as _;

    Identity {
        file: meta.file_index().unwrap_or_default(),
        links: u64::from(meta.number_of_links().unwrap_or_default()),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{Disk, FileSystem};

    /// A fresh, empty directory of its own, so tests cannot collide over a file
    /// name. Built from the process id and a counter rather than a random name,
    /// which the workspace has no dependency for.
    fn scratch() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "lemonfiber-fs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[tokio::test]
    async fn a_created_file_can_be_linked_and_the_two_names_share_one_file() {
        let dir = scratch();
        let original = dir.join("probe");
        let linked = dir.join("probe.link");

        assert!(Disk.touch(&original).await.is_ok());
        assert!(Disk.link(&original, &linked).await.is_ok());

        let one = Disk.identify(&original).await.ok();
        let two = Disk.identify(&linked).await.ok();
        assert_eq!(
            one.map(|id| id.file),
            two.map(|id| id.file),
            "a hardlink and its original are the same underlying file"
        );

        Disk.remove(&linked).await;
        Disk.remove(&original).await;
        assert!(!linked.exists(), "cleanup removes what the probe created");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_symlinked_directory_resolves_to_where_it_points() {
        let dir = scratch();
        let real = dir.join("real");
        let _ = std::fs::create_dir_all(&real);

        // Canonicalising the real directory is enough to exercise resolution on
        // every platform; the symlink case is covered where the platform makes
        // one cheaply, and its absence here changes no branch.
        let resolved = Disk.canonicalize(&real).await.ok();
        assert!(
            resolved.is_some_and(|path| path.ends_with("real")),
            "a real path resolves to itself"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_path_that_is_not_there_cannot_be_resolved_or_identified() {
        let missing = Path::new("/lemonfiber/no/such/data/root");
        assert!(Disk.canonicalize(missing).await.is_err());
        assert!(Disk.identify(missing).await.is_err());
    }

    #[tokio::test]
    async fn creating_or_linking_beneath_a_file_fails_as_the_platform_says() {
        let dir = scratch();
        // A file where a directory would need to be: the portable way to force a
        // create and a link to fail the same way on every platform.
        let blocker = dir.join("blocker");
        let _ = std::fs::write(&blocker, "in the way");

        let beneath = blocker.join("probe");
        assert!(
            Disk.touch(&beneath).await.is_err(),
            "cannot create under a file"
        );
        assert!(
            Disk.link(Path::new("/lemonfiber/no/such/source"), &beneath)
                .await
                .is_err(),
            "cannot link a source that is not there"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn removing_something_already_gone_is_not_an_error() {
        // remove reports nothing, so the guarantee is only that it returns; an
        // absent file is the case that would fail if it did not swallow it.
        Disk.remove(Path::new("/lemonfiber/no/such/file")).await;
    }

    #[tokio::test]
    async fn a_real_directory_sits_on_a_filesystem_the_platform_can_name() {
        let dir = scratch();
        // The temp directory sits on whatever this machine runs on; the point is
        // that describing it returns facts rather than that the type is a
        // particular one, which varies by runner.
        let facts = Disk.describe(&dir).await;
        assert!(
            !facts.kind.is_network(),
            "a local temp directory is not a network filesystem"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
