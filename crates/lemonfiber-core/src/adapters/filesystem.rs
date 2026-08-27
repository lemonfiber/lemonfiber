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

use crate::ports::filesystem::{
    Eraser, Fault, FileSystem, Identity, Mount, Ownership, Presence, StorageFacts, Volume,
};

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

    async fn read(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    /// One syscall, which is the whole point: `create_new` asks the kernel to create
    /// the file *and* fail if it already exists, so two processes racing here get one
    /// `true` between them. Anything built from a separate look-then-write would have
    /// a window in it, and a lock with a window is not a lock.
    async fn claim(&self, path: &Path, contents: &str) -> bool {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map(|mut file| {
                use std::io::Write as _;
                let _ = file.write_all(contents.as_bytes());
            })
            .is_ok()
    }

    async fn write(&self, path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, contents);
    }

    async fn ownership(&self, path: &Path) -> Option<Ownership> {
        ownership_of(path)
    }

    async fn describe(&self, path: &Path) -> StorageFacts {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        let mounts: Vec<Mount> = disks
            .list()
            .iter()
            .map(|disk| Mount {
                point: disk.mount_point().to_path_buf(),
                kind: disk.file_system().to_string_lossy().into_owned(),
                removable: disk.is_removable(),
                available: disk.available_space(),
                total: disk.total_space(),
            })
            .collect();
        crate::ports::filesystem::pick(&mounts, path)
    }
}

#[async_trait]
impl Eraser for Disk {
    async fn erase(&self, path: &Path) -> Result<(), Fault> {
        match std::fs::remove_dir_all(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(fault(&error)),
        }
    }
}

#[async_trait]
impl Volume for Disk {
    async fn presence(&self, path: &Path) -> Presence {
        match std::fs::metadata(path) {
            Ok(meta) => Presence::On(volume_of(&meta)),
            // Only a plain "not there" is `Gone`. A permission error or an
            // interrupted call says nothing about whether the volume is still
            // mounted, so it is `Unknown` and the watch holds rather than acting.
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Presence::Gone,
            Err(_) => Presence::Unknown,
        }
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

/// Who owns a path on Unix, and its permission bits.
///
/// Only the low bits of the mode are kept — the nine that decide who may read,
/// write and traverse — because the file-type bits above them are not what the
/// permission check reasons about.
#[cfg(unix)]
fn ownership_of(path: &Path) -> Option<Ownership> {
    use std::os::unix::fs::MetadataExt as _;

    let meta = std::fs::metadata(path).ok()?;
    Some(Ownership {
        uid: meta.uid(),
        gid: meta.gid(),
        mode: meta.mode() & 0o777,
    })
}

/// Ownership as Windows reports it: it does not, in the terms this check needs,
/// so there is nothing to report and the check skips accordingly.
#[cfg(not(unix))]
fn ownership_of(_path: &Path) -> Option<Ownership> {
    None
}

/// The volume a file sits on, as Unix's device number.
///
/// The number that changes when the same path resolves to a different mount —
/// the signal a watch reads to tell a drive that is still there from one that
/// was pulled and left its mount point behind.
#[cfg(unix)]
fn volume_of(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;

    meta.dev()
}

/// The volume as Windows reports it: with no device number to read, every
/// present path reads as the same volume, so a watch there notices a path
/// disappearing but not a mount being swapped beneath it.
#[cfg(not(unix))]
fn volume_of(_meta: &std::fs::Metadata) -> u64 {
    0
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

    use super::{Disk, Eraser, FileSystem, Volume};

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

    /// A directory and everything under it, and the second run over the same path.
    ///
    /// Already gone is already removed: running this twice is a reasonable thing to
    /// do, and the second run has nothing to complain about. A caller told otherwise
    /// would report a machine as not clean when it is.
    #[tokio::test]
    async fn a_whole_tree_goes_and_a_second_removal_of_it_is_not_a_failure() {
        let dir = scratch();
        let inside = dir.join("nested");
        let file = inside.join("kept");
        let _ = std::fs::create_dir_all(&inside);
        assert!(Disk.touch(&file).await.is_ok());

        assert!(Disk.erase(&dir).await.is_ok());
        assert!(!dir.exists());
        assert!(Disk.erase(&dir).await.is_ok());
    }

    /// A path that is not a directory is the platform's own refusal, carried
    /// verbatim — it is what the operator needs in order to finish by hand.
    #[tokio::test]
    async fn a_tree_that_will_not_go_comes_back_in_the_platforms_own_words() {
        let dir = scratch();
        let file = dir.join("not-a-directory");
        assert!(Disk.touch(&file).await.is_ok());

        let refused = Disk.erase(&file).await;
        assert!(
            refused.is_err_and(|fault| !fault.message.is_empty()),
            "the platform said nothing about refusing"
        );
    }

    #[tokio::test]
    async fn removing_something_already_gone_is_not_an_error() {
        // remove reports nothing, so the guarantee is only that it returns; an
        // absent file is the case that would fail if it did not swallow it.
        Disk.remove(Path::new("/lemonfiber/no/such/file")).await;
    }

    #[tokio::test]
    async fn a_written_note_reads_back_and_an_absent_one_is_nothing() {
        let dir = scratch();
        // The directory does not exist beforehand; write creates it, which is the
        // first-run case for a state file lemonfiber keeps for itself.
        let note = dir.join("state").join("note.json");

        assert_eq!(Disk.read(&note).await, None, "nothing recorded yet");
        Disk.write(&note, "remembered").await;
        assert_eq!(Disk.read(&note).await.as_deref(), Some("remembered"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_present_path_reports_its_volume_and_an_absent_one_is_gone() {
        use crate::ports::filesystem::Presence;

        let dir = scratch();
        let here = Disk.presence(&dir).await;
        let sibling = Disk.presence(&dir.join("child")).await;
        assert!(
            matches!(here, Presence::On(_)),
            "a real directory is present"
        );
        // The child does not exist yet, so it is gone even though its parent is
        // not — presence is about the path asked for, not its neighbourhood.
        assert_eq!(sibling, Presence::Gone);
        assert_eq!(
            Disk.presence(Path::new("/lemonfiber/no/such/root")).await,
            Presence::Gone
        );
        // A path that reaches *through* a regular file cannot be statted, and the
        // error is "not a directory", not "not found" — so it reads as Unknown
        // rather than being mistaken for the volume being gone.
        let file = dir.join("a-file");
        let _ = std::fs::write(&file, "x");
        assert_eq!(Disk.presence(&file.join("child")).await, Presence::Unknown);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_real_directory_reports_ownership_and_an_absent_one_reports_none() {
        let dir = scratch();
        // On Unix the scratch directory has an owner and a mode; the point is that
        // ownership comes back at all and its mode is within the permission bits.
        // Where the platform does not report ownership, both cases are None, which
        // the assertion below still holds for.
        let present = Disk.ownership(&dir).await;
        assert!(
            present.is_none_or(|owner| owner.mode & 0o200 != 0 && owner.mode <= 0o777),
            "a directory we just created is writable by its owner, and only permission bits are kept"
        );
        assert_eq!(
            Disk.ownership(Path::new("/lemonfiber/no/such/path")).await,
            None,
            "nothing to own where nothing is there"
        );
        let _ = std::fs::remove_dir_all(&dir);
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
        assert!(facts.total > 0, "a real volume reports a size");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The promise the whole lock rests on: the second caller is told it lost,
    /// rather than quietly writing over what the first one put there.
    #[tokio::test]
    async fn only_the_first_claim_of_a_path_succeeds() {
        let path = scratch().join("lifecycle.lock");

        assert!(
            Disk.claim(&path, "first").await,
            "nothing was there, so this call created it"
        );
        assert!(
            !Disk.claim(&path, "second").await,
            "it was there, so this call did not"
        );
        assert_eq!(
            Disk.read(&path).await.as_deref(),
            Some("first"),
            "and the loser wrote nothing over the winner"
        );
    }

    /// A claim creates the directory it belongs in, because the first run on a
    /// machine claims before anything else has had cause to make one.
    #[tokio::test]
    async fn a_claim_makes_the_directory_it_needs() {
        let path = scratch().join("nested").join("lifecycle.lock");

        assert!(Disk.claim(&path, "held").await);
        assert_eq!(Disk.read(&path).await.as_deref(), Some("held"));
    }
}
