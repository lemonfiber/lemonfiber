//! The file operations whose meaning differs from one platform to the next.
//!
//! Each is one call a caller makes the same way everywhere. Where Unix keeps a mode, an
//! owner, a change time or a flag that refuses a link and Windows does not, the answer
//! here is the nearest thing the platform has, or nothing, and the caller never asks
//! which it got.

use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::path::Path;

/// The mode of a file only its owner may read and write.
pub const OWNER_FILE: u32 = 0o600;

/// The mode of a directory only its owner may list, enter and write.
pub const OWNER_DIRECTORY: u32 = 0o700;

/// The mode of a file its owner writes and anybody may read.
pub const SHARED_FILE: u32 = 0o644;

/// The mode of a directory anybody may list, enter and write, before the umask.
pub const SHARED_DIRECTORY: u32 = 0o777;

/// The nine bits of a mode that decide who may read, write and traverse: none of
/// set-user, set-group or sticky, and none of the file-type bits above them.
pub const PERMISSION_BITS: u32 = 0o777;

/// Who owns a file and its mode, where the platform keeps both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Owned {
    /// The owning user's id.
    pub uid: u32,
    /// The owning group's id.
    pub gid: u32,
    /// The whole mode, file-type bits included.
    pub mode: u32,
}

/// Which file a reading is of, and how many names point at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Numbered {
    /// The file's number on its volume: an inode on Unix, a file index on Windows.
    pub file: u64,
    /// How many names point at the file.
    pub links: u64,
}

/// Make the directory `at` with `mode`, and every one missing above it where `parents`
/// says so. The mode is set as each is created, so one already there is left as it
/// was; where the platform keeps no mode, an ordinary create.
///
/// # Errors
///
/// Where the platform refuses the create.
pub fn directory(at: &Path, parents: bool, mode: u32) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(parents);
    with_mode(&mut builder, mode);
    builder.create(at)
}

/// Make the directory `at` private to its owner, as [`directory`] makes one with
/// [`OWNER_DIRECTORY`].
///
/// # Errors
///
/// Where the platform refuses the create.
pub fn private_directory(at: &Path, parents: bool) -> io::Result<()> {
    directory(at, parents, OWNER_DIRECTORY)
}

/// `options` making a file readable and writable by its owner alone where it creates
/// one, and unchanged where the platform keeps no mode.
pub fn owner_only(options: &mut OpenOptions) -> &mut OpenOptions {
    made_as(options, OWNER_FILE)
}

/// `options` refusing to open through a link at the last name of the path, and
/// unchanged where the platform has no flag for it.
pub fn unfollowed(options: &mut OpenOptions) -> &mut OpenOptions {
    flagged(options, false)
}

/// `options` refusing to open through a link at the last name of the path and refusing
/// to wait on a pipe put there, and unchanged where the platform has no flag for either.
pub fn unfollowed_unwaiting(options: &mut OpenOptions) -> &mut OpenOptions {
    flagged(options, true)
}

/// Whether two readings are of one file.
#[must_use]
pub fn same_file(one: &Metadata, other: &Metadata) -> bool {
    same(one, other)
}

#[cfg(unix)]
fn with_mode(builder: &mut std::fs::DirBuilder, mode: u32) {
    use std::os::unix::fs::DirBuilderExt as _;
    builder.mode(mode);
}

#[cfg(not(unix))]
fn with_mode(_builder: &mut std::fs::DirBuilder, _mode: u32) {}

#[cfg(unix)]
fn made_as(options: &mut OpenOptions, mode: u32) -> &mut OpenOptions {
    use std::os::unix::fs::OpenOptionsExt as _;
    options.mode(mode)
}

#[cfg(not(unix))]
fn made_as(options: &mut OpenOptions, _mode: u32) -> &mut OpenOptions {
    options
}

#[cfg(unix)]
fn flagged(options: &mut OpenOptions, unwaiting: bool) -> &mut OpenOptions {
    use std::os::unix::fs::OpenOptionsExt as _;
    let waiting = if unwaiting { libc::O_NONBLOCK } else { 0 };
    options.custom_flags(libc::O_NOFOLLOW | waiting)
}

#[cfg(not(unix))]
fn flagged(options: &mut OpenOptions, _unwaiting: bool) -> &mut OpenOptions {
    options
}

/// Hold the file `file` holds to `mode`, through its handle; nothing where the platform
/// keeps no mode.
///
/// # Errors
///
/// Where the platform refuses the change.
#[cfg(unix)]
pub fn set_mode(file: &File, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    file.set_permissions(std::fs::Permissions::from_mode(mode))
}

/// Hold the file `file` holds to `mode`, through its handle; nothing where the platform
/// keeps no mode.
///
/// # Errors
///
/// Where the platform refuses the change.
#[cfg(not(unix))]
pub fn set_mode(_file: &File, _mode: u32) -> io::Result<()> {
    Ok(())
}

/// Give `at` `mode`, by its name; nothing where the platform keeps no mode.
///
/// # Errors
///
/// Where the platform refuses the change.
#[cfg(unix)]
pub fn set_mode_at(at: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(at, std::fs::Permissions::from_mode(mode))
}

/// Give `at` `mode`, by its name; nothing where the platform keeps no mode.
///
/// # Errors
///
/// Where the platform refuses the change.
#[cfg(not(unix))]
pub fn set_mode_at(_at: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}

/// Flush the directory `at`, so a rename into it survives a power cut and not only the
/// bytes it moved; nothing where a directory cannot be opened to be flushed, and the
/// platform's own rename is what there is.
///
/// # Errors
///
/// Where the directory would not open or flush.
#[cfg(unix)]
pub fn sync_directory(at: &Path) -> io::Result<()> {
    File::open(at).and_then(|directory| directory.sync_all())
}

/// Flush the directory `at`, so a rename into it survives a power cut and not only the
/// bytes it moved; nothing where a directory cannot be opened to be flushed, and the
/// platform's own rename is what there is.
///
/// # Errors
///
/// Where the directory would not open or flush.
#[cfg(not(unix))]
pub fn sync_directory(_at: &Path) -> io::Result<()> {
    Ok(())
}

/// When the file `meta` reads last changed, in nanoseconds since the epoch: its change
/// time, which moves on a write and on a change of mode or owner alike.
#[cfg(unix)]
#[must_use]
pub fn changed(meta: &Metadata) -> Option<u128> {
    use std::os::unix::fs::MetadataExt as _;
    let seconds = u128::try_from(meta.ctime()).ok()?;
    let nanos = u128::try_from(meta.ctime_nsec()).ok()?;
    Some(seconds * 1_000_000_000 + nanos)
}

/// When the file `meta` reads was last written, in nanoseconds since the epoch, where
/// the platform keeps no change time.
#[cfg(not(unix))]
#[must_use]
pub fn changed(meta: &Metadata) -> Option<u128> {
    let written = meta.modified().ok()?;
    Some(
        written
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos(),
    )
}

/// Who owns the file `meta` reads, and its mode.
#[cfg(unix)]
#[must_use]
pub fn owned(meta: &Metadata) -> Option<Owned> {
    use std::os::unix::fs::MetadataExt as _;
    Some(Owned {
        uid: meta.uid(),
        gid: meta.gid(),
        mode: meta.mode(),
    })
}

/// Nobody, where the platform names no owner in the terms Unix does.
#[cfg(not(unix))]
#[must_use]
pub fn owned(_meta: &Metadata) -> Option<Owned> {
    None
}

/// Give `at` to the user `uid` and the group `gid`, without following a link at it.
///
/// # Errors
///
/// Where the platform refuses the change.
#[cfg(unix)]
pub fn hand_over(at: &Path, uid: u32, gid: u32) -> io::Result<()> {
    std::os::unix::fs::lchown(at, Some(uid), Some(gid))
}

/// Nothing to hand over, where the platform names no owner.
///
/// # Errors
///
/// None.
#[cfg(not(unix))]
pub fn hand_over(_at: &Path, _uid: u32, _gid: u32) -> io::Result<()> {
    Ok(())
}

/// The volume the file `meta` reads sits on, as its device number: the number that
/// changes when the same path resolves to a different mount.
#[cfg(unix)]
#[must_use]
pub fn volume(meta: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;
    meta.dev()
}

/// The volume, where there is no device number to read: every present path reads as
/// the same one, so a watch notices a path disappearing but not a mount being swapped
/// beneath it.
#[cfg(not(unix))]
#[must_use]
pub fn volume(_meta: &Metadata) -> u64 {
    0
}

/// The file `meta` reads, as Unix numbers it: its inode and how many names point at it.
#[cfg(unix)]
#[must_use]
pub fn numbered(meta: &Metadata) -> Numbered {
    use std::os::unix::fs::MetadataExt as _;
    Numbered {
        file: meta.ino(),
        links: meta.nlink(),
    }
}

/// The file `meta` reads, as Windows numbers it: its file index, and its link count
/// where the filesystem supplies one.
///
/// A filesystem that reports no index leaves nothing to compare, which the caller
/// reads as being unable to confirm a link rather than as a failure to make one.
#[cfg(windows)]
#[must_use]
pub fn numbered(meta: &Metadata) -> Numbered {
    use std::os::windows::fs::MetadataExt as _;
    Numbered {
        file: meta.file_index().unwrap_or_default(),
        links: u64::from(meta.number_of_links().unwrap_or_default()),
    }
}

#[cfg(unix)]
fn same(one: &Metadata, other: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    one.dev() == other.dev() && one.ino() == other.ino()
}

#[cfg(windows)]
fn same(one: &Metadata, other: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    one.volume_serial_number() == other.volume_serial_number()
        && one.file_index() == other.file_index()
        && one.file_index().is_some()
}

#[cfg(not(any(unix, windows)))]
fn same(one: &Metadata, other: &Metadata) -> bool {
    one.len() == other.len()
}

/// Make a link at `link` to the directory `original`.
///
/// # Errors
///
/// Where the platform refuses the link, or keeps none.
#[cfg(unix)]
pub fn link_directory(original: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(original, link)
}

/// Make a link at `link` to the directory `original`.
///
/// # Errors
///
/// Where the platform refuses the link, or keeps none.
#[cfg(not(unix))]
pub fn link_directory(_original: &Path, _link: &Path) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(test)]
mod tests;
