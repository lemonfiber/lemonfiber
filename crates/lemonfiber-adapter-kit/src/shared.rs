//! A file in the configuration directory an adapter shares with the upstream it fronts,
//! written and read without following a link.
//!
//! The upstream can write that directory too, so any name in it may be a link it planted
//! to send a write elsewhere. The directory is opened once, refusing a link there, and
//! every step after it works on that handle: a stale staging name is unlinked, which
//! removes a link and never follows it; the staging file is created new, refusing to
//! follow one; its mode is set and it is flushed through its own handle; and it is
//! renamed over the target, which replaces a link rather than writing through it; no
//! file is ever opened in place to be written. A read refuses anything but a plain file
//! with one name before opening it, opens it without following a link and without
//! blocking, refuses it again through the handle before reading a byte, and is bounded.
//! A name is a [`SharedName`], one path component, built only by validating it.

use std::io::{Read as _, Write as _};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use rustix::fs::{
    fchmod, fstat, fsync, openat, renameat, statat, unlinkat, AtFlags, FileType, Mode, OFlags,
    Stat, CWD,
};
use rustix::io::Errno;

/// The largest file read, so a file the upstream grew without bound is refused rather
/// than held.
pub const LARGEST: u64 = 4 * 1024 * 1024;

/// The bits of a mode a shared file is written with: the owner's write and everyone's
/// read.
const KEPT: u32 = 0o644;

/// The longest file name, in bytes, one path component holds.
const LONGEST_NAME: usize = 255;

/// What a staging file is named beside its target: hidden, and never a target's name.
const STAGING: &str = ".writing";

/// Why a shared file was not written or read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unshared {
    /// The name is not one path component.
    Named(String),
    /// The directory, or the file named, is a link or not what was asked for: a
    /// directory that is not one, a file that is not a plain file, or a file that has
    /// another name too.
    Linked(String),
    /// The file is larger than [`LARGEST`].
    Large(String),
    /// The system refused a step, naming the file and why.
    Failed(String, String),
}

impl std::fmt::Display for Unshared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Named(name) => write!(f, "{name:?} is not one plain file name"),
            Self::Linked(name) => write!(f, "{name} is a link or not a plain file"),
            Self::Large(name) => write!(f, "{name} is larger than {LARGEST} bytes"),
            Self::Failed(name, why) => write!(f, "{name} could not be reached: {why}"),
        }
    }
}

impl std::error::Error for Unshared {}

/// One file name in the shared directory: exactly one path component, and never a
/// staging name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedName(String);

impl SharedName {
    /// `name`, where it is one path component: not empty and no longer than a file
    /// name is, not `.` or `..`, with no `/` and no NUL, and not ending as a staging
    /// name does.
    ///
    /// # Errors
    ///
    /// [`Unshared::Named`] for anything else.
    pub fn new(name: &str) -> Result<Self, Unshared> {
        let one = !name.is_empty()
            && name.len() <= LONGEST_NAME
            && name != "."
            && name != ".."
            && !name.contains(['/', '\0'])
            && !name.ends_with(STAGING);
        if one {
            Ok(Self(name.to_owned()))
        } else {
            Err(Unshared::Named(name.to_owned()))
        }
    }

    /// The name as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The configuration directory an adapter shares with its upstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shared {
    directory: PathBuf,
}

impl Shared {
    /// The directory shared at `directory`.
    #[must_use]
    pub fn at(directory: &Path) -> Self {
        Self {
            directory: directory.to_path_buf(),
        }
    }

    /// Write `contents` as the whole of the file `name` with `mode`, of which only the
    /// owner's write and everyone's read are kept, replacing whatever stood at the name
    /// and following no link.
    ///
    /// # Errors
    ///
    /// [`Unshared`] where the directory is a link or a step fails; nothing outside the
    /// directory is written.
    pub async fn write(
        &self,
        name: &SharedName,
        contents: Vec<u8>,
        mode: u32,
    ) -> Result<(), Unshared> {
        let directory = self.directory.clone();
        let owned = name.clone();
        tokio::task::spawn_blocking(move || written(&directory, owned.as_str(), &contents, mode))
            .await
            .map_err(failed(name.as_str()))?
    }

    /// What the file `name` holds, or none where nothing is there, following no link.
    ///
    /// # Errors
    ///
    /// [`Unshared`] where the directory or the file is a link, the file is not a plain
    /// file with one name, it is larger than [`LARGEST`], or a step fails.
    pub async fn read(&self, name: &SharedName) -> Result<Option<Vec<u8>>, Unshared> {
        let directory = self.directory.clone();
        let owned = name.clone();
        tokio::task::spawn_blocking(move || read(&directory, owned.as_str()))
            .await
            .map_err(failed(name.as_str()))?
    }
}

/// What a step that failed on the file `name` is reported as.
fn failed<E: std::fmt::Display>(name: &str) -> impl FnOnce(E) -> Unshared + '_ {
    move |why| Unshared::Failed(name.to_owned(), why.to_string())
}

/// The directory at `path`, opened once and refused where it is a link.
fn opened(path: &Path) -> Result<OwnedFd, Unshared> {
    let shown = path.display().to_string();
    openat(
        CWD,
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|why| match why {
        Errno::LOOP | Errno::NOTDIR => Unshared::Linked(shown),
        other => Unshared::Failed(shown, other.to_string()),
    })
}

/// Write `contents` to `name` in the directory at `path`, through its handle alone.
fn written(path: &Path, name: &str, contents: &[u8], mode: u32) -> Result<(), Unshared> {
    let directory = opened(path)?;
    let staging = format!(".{name}{STAGING}");
    match unlinkat(&directory, staging.as_str(), AtFlags::empty()) {
        Ok(()) | Err(Errno::NOENT) => {}
        Err(Errno::ISDIR | Errno::PERM) => return Err(Unshared::Linked(staging)),
        Err(why) => return Err(failed(name)(why)),
    }
    let permitted =
        Mode::from_bits_truncate(rustix::fs::RawMode::try_from(mode & KEPT).unwrap_or_default());
    let created = openat(
        &directory,
        staging.as_str(),
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        permitted,
    )
    .map_err(failed(name))?;
    let moved = filled(created, name, contents, permitted).and_then(|()| {
        renameat(&directory, staging.as_str(), &directory, name).map_err(failed(name))
    });
    if moved.is_err() {
        let _ = unlinkat(&directory, staging.as_str(), AtFlags::empty());
        return moved;
    }
    let _ = fsync(&directory);
    Ok(())
}

/// Write `contents` into the file `name` just created, set its mode and flush it.
fn filled(created: OwnedFd, name: &str, contents: &[u8], mode: Mode) -> Result<(), Unshared> {
    alone(&fstat(&created).map_err(failed(name))?, name)?;
    fchmod(&created, mode).map_err(failed(name))?;
    let mut file = std::fs::File::from(created);
    file.write_all(contents).map_err(failed(name))?;
    fsync(&file).map_err(failed(name))
}

/// Refuse the file `name` described by `held` unless it is a plain file with no other
/// name, so neither a link, a device, a pipe nor a file hardlinked in from elsewhere is
/// read or written.
fn alone(held: &Stat, name: &str) -> Result<(), Unshared> {
    if FileType::from_raw_mode(held.st_mode) == FileType::RegularFile && held.st_nlink == 1 {
        Ok(())
    } else {
        Err(Unshared::Linked(name.to_owned()))
    }
}

/// What `name` in the directory at `path` holds, through its handle alone: looked at
/// without following a link and refused before it is opened unless it is a plain file
/// with one name, then looked at again through the handle opened, and read bounded.
fn read(path: &Path, name: &str) -> Result<Option<Vec<u8>>, Unshared> {
    let directory = opened(path)?;
    match statat(&directory, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(seen) => alone(&seen, name)?,
        Err(Errno::NOENT) => return Ok(None),
        Err(why) => return Err(failed(name)(why)),
    }
    let file = openat(
        &directory,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(failed(name))?;
    alone(&fstat(&file).map_err(failed(name))?, name)?;
    let mut bytes = Vec::new();
    std::fs::File::from(file)
        .take(LARGEST + 1)
        .read_to_end(&mut bytes)
        .map_err(failed(name))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > LARGEST {
        return Err(Unshared::Large(name.to_owned()));
    }
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests;
