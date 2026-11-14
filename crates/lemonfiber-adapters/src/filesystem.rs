//! Touching the real filesystem.
//!
//! Translation, and no decisions. Each method is one system operation whose
//! result — success, or the platform's own words for a failure — is handed
//! straight back; what a sequence of them proves is decided in
//! the core's own storage diagnosis, where a fake filesystem can drive every outcome.
//!
//! What differs by platform — the identity a file reports, its owner and mode, the
//! flag that refuses a link — is asked of `lemonfiber_platform`, so nothing here asks
//! what it is running on.
//!
//! `sysinfo` lives here and nowhere else, which an architecture test enforces.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_platform::fs;
use sysinfo::{DiskRefreshKind, Disks};

use lemonfiber_ports::filesystem::{
    pick, Beneath, Eraser, Fault, FileSystem, Identity, Mount, Ownership, Presence, Storage,
    StorageFacts, Volume,
};

/// How long the platform is given to describe the volume behind a path.
///
/// Five seconds, against a reading that takes milliseconds. A network mount whose
/// server has gone away can leave the read waiting without end, and an answer of
/// nothing known is better than a caller that never gets one.
const DESCRIBED_WITHIN: Duration = Duration::from_secs(5);

/// The filesystem on this machine, reached through the standard library.
#[derive(Debug, Default, Clone, Copy)]
pub struct Disk;

#[async_trait]
impl FileSystem for Disk {
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf, Fault> {
        tokio::fs::canonicalize(path)
            .await
            .map_err(|error| fault(&error))
    }

    /// Created new, which the kernel refuses where anything is at the name — a link
    /// included, so nothing a link points at is created or emptied through it.
    async fn touch(&self, path: &Path) -> Result<(), Fault> {
        tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .await
            .map(drop)
            .map_err(|error| fault(&error))
    }

    async fn link(&self, from: &Path, to: &Path) -> Result<(), Fault> {
        tokio::fs::hard_link(from, to)
            .await
            .map_err(|error| fault(&error))
    }

    async fn identify(&self, path: &Path) -> Result<Identity, Fault> {
        tokio::fs::metadata(path)
            .await
            .map(|meta| identity_of(&meta))
            .map_err(|error| fault(&error))
    }

    async fn remove(&self, path: &Path) {
        let _ = tokio::fs::remove_file(path).await;
    }

    async fn read(&self, path: &Path) -> Option<String> {
        tokio::fs::read_to_string(path).await.ok()
    }

    /// The confined read, on a thread that may block: it never waits on a
    /// pipe, but it is still a disk read.
    async fn read_beneath(&self, path: &Path, within: &Path) -> Beneath {
        let (path, within) = (path.to_path_buf(), within.to_path_buf());
        tokio::task::spawn_blocking(move || confined::read(&path, &within))
            .await
            .unwrap_or(Beneath::Absent)
    }

    async fn make_beneath(&self, path: &Path, within: &Path) -> Result<(), Fault> {
        let (path, within) = (path.to_path_buf(), within.to_path_buf());
        tokio::task::spawn_blocking(move || confined::make(&path, &within))
            .await
            .unwrap_or(Err(Fault::new("the directory was not made")))
    }

    /// One syscall, which is the whole point: `create_new` asks the kernel to create
    /// the file *and* fail if it already exists, so two processes racing here get one
    /// `true` between them. Anything built from a separate look-then-write would have
    /// a window in it, and a lock with a window is not a lock.
    async fn claim(&self, path: &Path, contents: &str) -> bool {
        claimed(path, contents).await
    }

    async fn write(&self, path: &Path, contents: &str) {
        written(path, contents).await;
    }

    async fn ownership(&self, path: &Path) -> Option<Ownership> {
        ownership_of(path)
    }
}

/// Create the file and write it, or say somebody else got there first.
///
/// The file existing is the claim, and what is written in it only names the holder to
/// whoever is refused. So a claim whose contents would not write is still held: giving
/// it up would leave a file nobody holds in the way of every run, or a window for a
/// second one, and a holder that says nothing is a case the reader already handles.
///
/// Flushed before it is reported won. A tokio file hands each write to a blocking
/// task and returns, so an unflushed claim can be read back empty by the next run.
/// Not synced to disk: after a crash the claim stands until `--force` whatever it
/// says, and all its contents decide is the wording of a refusal.
async fn claimed(path: &Path, contents: &str) -> bool {
    use tokio::io::AsyncWriteExt as _;
    made_room_for(path).await;
    let opened = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await;
    let Ok(mut file) = opened else {
        return false;
    };
    let _ = file.write_all(contents.as_bytes()).await;
    let _ = file.flush().await;
    true
}

/// Write the file, making its directory first where it has one.
///
/// Owner-only where the platform tracks a file mode, set as the file is created and
/// again on one that was there: what lands here is lemonfiber's own record or an export
/// of the stack's logs, and neither is for another user on the same machine to read.
async fn written(path: &Path, contents: &str) {
    made_room_for(path).await;
    let _ = private(path, contents).await;
}

/// Write `contents` to a file readable by its owner alone, where the platform keeps a
/// mode.
///
/// Never through a link at its name, and tightened through the handle rather than by the
/// name, so a link put there after the open cannot turn the mode change onto another file.
async fn private(path: &Path, contents: &str) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    fs::owner_only(&mut options);
    fs::unfollowed(&mut options);
    let mut file = tokio::fs::OpenOptions::from(options).open(path).await?;
    file.write_all(contents.as_bytes()).await?;
    file.flush().await?;
    let file = file.into_std().await;
    tokio::task::spawn_blocking(move || fs::set_mode(&file, fs::OWNER_FILE))
        .await
        .map_err(std::io::Error::other)?
}

/// The directory a file is about to go in, where the path names one.
///
/// Best-effort on purpose: a directory that cannot be made is a write that is about
/// to fail and say so itself, which is a better answer than one from here about a
/// directory the caller never mentioned.
async fn made_room_for(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
}

#[async_trait]
impl Storage for Disk {
    /// Read on a thread that may block, and for [`DESCRIBED_WITHIN`] at most.
    ///
    /// Every read here is a system call that can wait on a disk or a server, and on
    /// the runtime's own thread that wait is every other task's. A reading that runs
    /// out of time describes nothing, which is what a path no mount could be found
    /// for already reads as.
    async fn describe(&self, path: &Path) -> StorageFacts {
        let asked = path.to_path_buf();
        let reading = tokio::task::spawn_blocking(move || described(&asked));
        let read = tokio::time::timeout(DESCRIBED_WITHIN, reading).await;
        read.ok()
            .and_then(Result::ok)
            .unwrap_or_else(StorageFacts::unknown)
    }
}

/// The volume behind a path, with only that volume's space read.
///
/// The mounts are listed without reading any of their sizes, and only the one the
/// path sits on is then asked for its space. Reading every mount's size asks every
/// mount on the machine, and one that will not answer holds up the answer about a
/// path that was never on it.
fn described(path: &Path) -> StorageFacts {
    let mut disks: Vec<sysinfo::Disk> =
        Disks::new_with_refreshed_list_specifics(DiskRefreshKind::nothing())
            .map(Vec::from)
            .unwrap_or_default();
    let chosen = disks
        .iter_mut()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().components().count());
    let mounts: Vec<Mount> = chosen
        .into_iter()
        .map(|disk| {
            disk.refresh_specifics(DiskRefreshKind::nothing().with_storage());
            Mount {
                point: disk.mount_point().to_path_buf(),
                kind: disk.file_system().to_string_lossy().into_owned(),
                removable: disk.is_removable(),
                available: disk.available_space(),
                total: disk.total_space(),
            }
        })
        .collect();
    pick(&mounts, path)
}

#[async_trait]
impl Eraser for Disk {
    /// A tree or a single file, decided by asking rather than by trying.
    ///
    /// The port says "this path and everything beneath it", and a path with nothing
    /// beneath it is a file — which the tree removal refuses with a message about
    /// directories that would reach the operator as though something were wrong with
    /// their disk. Read first and remove accordingly: what is not there is already
    /// removed, and anything the metadata read itself refuses is the platform's own
    /// answer about a path nobody can act on.
    async fn erase(&self, path: &Path) -> Result<(), Fault> {
        erased(path).await
    }
}

/// The reading and the removal that [`Eraser::erase`] is.
async fn erased(path: &Path) -> Result<(), Fault> {
    let removed = match tokio::fs::symlink_metadata(path).await {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(fault(&error)),
        Ok(meta) if meta.is_dir() => tokio::fs::remove_dir_all(path).await,
        Ok(_) => tokio::fs::remove_file(path).await,
    };
    gone(removed)
}

/// What a removal's answer means, once it has been given.
///
/// Its own function because one of its three answers cannot be staged: a path that was
/// there when the metadata was read and gone when the removal ran is a race with
/// whatever else removed it. Handed the answer directly, that case is an ordinary test
/// rather than a thing nobody can reach.
fn gone(removed: std::io::Result<()>) -> Result<(), Fault> {
    match removed {
        Ok(()) => Ok(()),
        // Something else removed it between the reading and the removal, which is
        // the outcome asked for either way.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(fault(&error)),
    }
}

#[async_trait]
impl Volume for Disk {
    async fn presence(&self, path: &Path) -> Presence {
        present(path).await
    }
}

/// Whether the volume behind a path is still there, and which one it is.
///
/// Only a plain "not there" is `Gone`. A permission error or an interrupted call says
/// nothing about whether the volume is still mounted, so it is `Unknown` and the watch
/// holds rather than acting.
async fn present(path: &Path) -> Presence {
    match tokio::fs::metadata(path).await {
        Ok(meta) => Presence::On(volume_of(&meta)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Presence::Gone,
        Err(_) => Presence::Unknown,
    }
}

/// A filesystem error in the platform's own words.
fn fault(error: &std::io::Error) -> Fault {
    Fault::new(error.to_string())
}

/// A file's identity as the platform numbers it: its number on its volume and how many
/// names point at it.
pub(super) fn identity_of(meta: &std::fs::Metadata) -> Identity {
    let numbered = fs::numbered(meta);
    Identity {
        file: numbered.file,
        links: numbered.links,
    }
}

/// Who owns a path, and its permission bits, where the platform keeps both; nothing
/// where it does not, and the check skips accordingly.
///
/// Only the low bits of the mode are kept — the nine that decide who may read,
/// write and traverse — because the file-type bits above them are not what the
/// permission check reasons about.
fn ownership_of(path: &Path) -> Option<Ownership> {
    let meta = std::fs::metadata(path).ok()?;
    let owned = fs::owned(&meta)?;
    Some(Ownership {
        uid: owned.uid,
        gid: owned.gid,
        mode: owned.mode & fs::PERMISSION_BITS,
    })
}

/// The volume a file sits on: the number that changes when the same path resolves to a
/// different mount, which a watch reads to tell a drive that is still there from one
/// that was pulled and left its mount point behind.
fn volume_of(meta: &std::fs::Metadata) -> u64 {
    fs::volume(meta)
}

mod confined;

pub(crate) use confined::{unpacked, UNPACKED_AT_MOST};

#[cfg(test)]
mod tests;
