//! Touching the real filesystem.
//!
//! Translation, and no decisions. Each method is one system operation whose
//! result — success, or the platform's own words for a failure — is handed
//! straight back; what a sequence of them proves is decided in
//! the core's own storage diagnosis, where a fake filesystem can drive every outcome.
//!
//! The identity a file reports differs by platform — an inode on Unix, a file
//! index on Windows — so reading it is the one place here that asks what it is
//! running on. It asks with `cfg(unix)`/`cfg(windows)`, which selects the code
//! at compile time rather than testing the operating system at runtime.
//!
//! `sysinfo` lives here and nowhere else, which an architecture test enforces.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use lemonfiber_ports::filesystem::{
    Beneath, Eraser, Fault, FileSystem, Identity, Mount, Ownership, Presence, Storage,
    StorageFacts, Volume,
};

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

    async fn touch(&self, path: &Path) -> Result<(), Fault> {
        tokio::fs::File::create(path)
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

    /// Opened once, refusing a link at the file itself, and then checked by what was
    /// opened rather than by its name: the name, resolved, has to lie beneath `within`
    /// and to name the very file the handle holds. A link swapped in on the way after
    /// the open resolves somewhere else, or names another file, and either is refused —
    /// so there is no moment between a look and a read for it to change under.
    async fn read_beneath(&self, path: &Path, within: &Path) -> Beneath {
        read_beneath(path, within).await
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

/// Write `contents` to a file readable by its owner alone.
#[cfg(unix)]
async fn private(path: &Path, contents: &str) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    use tokio::io::AsyncWriteExt as _;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .await?;
    file.write_all(contents.as_bytes()).await?;
    file.flush().await?;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).await
}

/// Where the platform tracks no file mode, an ordinary write.
#[cfg(not(unix))]
async fn private(path: &Path, contents: &str) -> std::io::Result<()> {
    tokio::fs::write(path, contents).await
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
        lemonfiber_ports::filesystem::pick(&mounts, path)
    }
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

/// A file's identity as Unix reports it: its inode and how many names point at
/// it.
#[cfg(unix)]
pub(super) fn identity_of(meta: &std::fs::Metadata) -> Identity {
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
pub(super) fn identity_of(meta: &std::fs::Metadata) -> Identity {
    use std::os::windows::fs::MetadataExt as _;

    Identity {
        file: meta.file_index().unwrap_or_default(),
        links: u64::from(meta.number_of_links().unwrap_or_default()),
    }
}

/// Read `path`, a plain file beneath `within`, opening it once and checking what was
/// opened.
async fn read_beneath(path: &Path, within: &Path) -> Beneath {
    use tokio::io::AsyncReadExt as _;

    let Ok(mut file) = no_follow().open(path).await else {
        return unopened(path).await;
    };
    let (Ok(opened), Ok(root), Ok(resolved)) = (
        file.metadata().await,
        tokio::fs::canonicalize(within).await,
        tokio::fs::canonicalize(path).await,
    ) else {
        return Beneath::Absent;
    };
    let named = tokio::fs::metadata(&resolved).await;
    let plain = opened.is_file() && named.is_ok_and(|named| same_file(&opened, &named));
    if !plain || !resolved.starts_with(&root) {
        return Beneath::Escaped;
    }
    let mut text = String::new();
    match file.read_to_string(&mut text).await {
        Ok(_) => Beneath::Read(text),
        Err(_) => Beneath::Absent,
    }
}

/// Why a file that would not open was not read: a link at its last name, which the open
/// refused to follow, or nothing there to open at all.
async fn unopened(path: &Path) -> Beneath {
    match tokio::fs::symlink_metadata(path).await {
        Ok(meta) if meta.file_type().is_symlink() => Beneath::Escaped,
        Ok(_) | Err(_) => Beneath::Absent,
    }
}

/// Opening for reading, refusing to follow a link at the last name of the path.
#[cfg(unix)]
fn no_follow() -> tokio::fs::OpenOptions {
    let mut options = tokio::fs::OpenOptions::new();
    // Not blocking, as well: a pipe put where the file is expected would otherwise hold
    // the open until something wrote to it. A plain file is opened the same either way.
    options
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    options
}

/// Opening for reading. Windows has no flag for it, so a link at the last name is
/// caught by the file it opened not being the one the name resolves to.
#[cfg(windows)]
fn no_follow() -> tokio::fs::OpenOptions {
    let mut options = tokio::fs::OpenOptions::new();
    options.read(true);
    options
}

/// Whether two readings are of one file: the same device and the same number on it.
#[cfg(unix)]
fn same_file(one: &std::fs::Metadata, other: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    one.dev() == other.dev() && one.ino() == other.ino()
}

/// Whether two readings are of one file: the same volume and the same index on it.
#[cfg(windows)]
fn same_file(one: &std::fs::Metadata, other: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    one.volume_serial_number() == other.volume_serial_number()
        && one.file_index() == other.file_index()
        && one.file_index().is_some()
}

#[cfg(test)]
mod tests;
